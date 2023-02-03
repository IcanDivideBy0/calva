#![warn(clippy::all)]

use anyhow::{anyhow, Result};
use renderer::{
    wgpu, AnimationId, AnimationState, AnimationsManager, Engine, Instance, Material, MaterialId,
    MeshId, PointLight, TextureId,
};
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::time::Duration;

mod animation;
use animation::*;

pub struct GltfModel {
    pub instances: Vec<Instance>,
    pub animations: HashMap<String, AnimationId>,
    pub point_lights: Vec<PointLight>,
}

impl GltfModel {
    pub fn from_reader(engine: &mut Engine, reader: &mut dyn Read) -> Result<Self> {
        let mut gltf_buffer = Vec::new();
        reader.read_to_end(&mut gltf_buffer)?;

        let (doc, buffers, images) = gltf::import_slice(&gltf_buffer)?;

        Self::new(engine, &doc, &buffers, &images)
    }

    pub fn new(
        engine: &mut Engine,
        doc: &gltf::Document,
        buffers: &[gltf::buffer::Data],
        images: &[gltf::image::Data],
    ) -> Result<Self> {
        let textures = Self::build_textures(engine, doc, images)?;

        let materials: Vec<MaterialId> = Self::build_materials(engine, doc, &textures)?;

        let meshes = Self::build_meshes(engine, doc, buffers)?;

        let nodes_transforms = Self::build_nodes_transforms(doc);

        let skins_animations = Self::build_skin_animations(engine, doc, &nodes_transforms, buffers);

        let instances: Vec<Instance> = doc
            .nodes()
            .filter_map(|node| {
                let mesh = node.mesh()?;
                let primitives = meshes.get(mesh.index())?;
                let transform = nodes_transforms.get(&node.index()).copied()?;

                let animation_state = node
                    .skin()
                    .and_then(|skin| skins_animations.get(skin.index()))
                    .and_then(|animations| animations.get("roar"))
                    .map(|&animation| AnimationState {
                        animation,
                        time: 0.0,
                    })
                    .unwrap_or_default();

                let mesh_instances = mesh
                    .primitives()
                    .zip(primitives)
                    .map(|(primitive, &mesh_id)| {
                        let material_id = primitive
                            .material()
                            .index()
                            .and_then(|index| materials.get(index).copied())
                            .unwrap_or_default();

                        Instance {
                            transform,
                            mesh: mesh_id,
                            material: material_id,
                            animation: animation_state,
                        }
                    })
                    .collect::<Vec<_>>();

                Some(mesh_instances)
            })
            .flatten()
            .collect();

        let point_lights: Vec<PointLight> = doc
            .nodes()
            .filter_map(|node| {
                use gltf::khr_lights_punctual::Kind;
                let light = node.light()?;

                match light.kind() {
                    Kind::Directional => {
                        unimplemented!();
                    }
                    Kind::Point => {
                        let color = light.color().into();
                        let position = nodes_transforms
                            .get(&node.index())?
                            .transform_point3(glam::Vec3::ZERO);
                        let radius = light.range().unwrap_or_else(|| light.intensity().sqrt());

                        Some(PointLight {
                            color,
                            position,
                            radius,
                        })
                    }
                    Kind::Spot { .. } => {
                        unimplemented!();
                    }
                }
            })
            .collect();

        Ok(Self {
            instances,
            animations: skins_animations.get(0).cloned().unwrap_or_default(),
            point_lights,
        })
    }

    fn build_textures(
        engine: &mut Engine,
        doc: &gltf::Document,
        images: &[gltf::image::Data],
    ) -> Result<Vec<TextureId>> {
        doc.textures()
            .map(|texture| {
                let image_data = images
                    .get(texture.source().index())
                    .ok_or_else(|| anyhow!("Invalid texture image index"))?;

                // 3 channels texture formats are not supported by WebGPU
                // https://github.com/gpuweb/gpuweb/issues/66
                let buf = if image_data.format == gltf::image::Format::R8G8B8 {
                    image::ImageBuffer::from_raw(
                        image_data.width,
                        image_data.height,
                        image_data.pixels.clone(),
                    )
                    .map(image::DynamicImage::ImageRgb8)
                } else {
                    image::ImageBuffer::from_raw(
                        image_data.width,
                        image_data.height,
                        image_data.pixels.clone(),
                    )
                    .map(image::DynamicImage::ImageRgba8)
                }
                .ok_or_else(|| anyhow!("Invalid image buffer"))?;

                let size = wgpu::Extent3d {
                    width: buf.width(),
                    height: buf.height(),
                    depth_or_array_layers: 1,
                };

                let dimension = wgpu::TextureDimension::D2;
                let desc = wgpu::TextureDescriptor {
                    label: texture.name(),
                    size,
                    mip_level_count: size.max_mips(dimension),
                    sample_count: 1,
                    dimension,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
                };

                let texture = engine.renderer.device.create_texture(&desc);

                engine.renderer.queue.write_texture(
                    texture.as_image_copy(),
                    &buf.to_rgba8(),
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: std::num::NonZeroU32::new(4 * size.width),
                        rows_per_image: None,
                    },
                    size,
                );

                engine.textures.generate_mipmaps(
                    &engine.renderer.device,
                    &engine.renderer.queue,
                    &texture,
                    &desc,
                )?;

                Ok(engine.textures.add(
                    &engine.renderer.device,
                    texture.create_view(&Default::default()),
                ))
            })
            .collect()
    }

    fn build_materials(
        engine: &mut Engine,
        doc: &gltf::Document,
        textures: &[TextureId],
    ) -> Result<Vec<MaterialId>> {
        doc.materials()
            .map(|material| {
                let albedo = material
                    .pbr_metallic_roughness()
                    .base_color_texture()
                    .and_then(|t| textures.get(t.texture().index()).copied())
                    .unwrap_or_default();

                let normal = material
                    .normal_texture()
                    .and_then(|t| textures.get(t.texture().index()).copied())
                    .unwrap_or_default();

                let metallic_roughness = material
                    .pbr_metallic_roughness()
                    .metallic_roughness_texture()
                    .and_then(|t| textures.get(t.texture().index()).copied())
                    .unwrap_or_default();

                Ok(engine.materials.add(
                    &engine.renderer.queue,
                    Material {
                        albedo,
                        normal,
                        metallic_roughness,
                    },
                ))
            })
            .collect()
    }

    fn build_meshes(
        engine: &mut Engine,
        doc: &gltf::Document,
        buffers: &[gltf::buffer::Data],
    ) -> Result<Vec<Vec<MeshId>>> {
        doc.meshes()
            .map(|mesh| {
                mesh.primitives()
                    .map(|primitive| {
                        let get_accessor_data = |accessor: gltf::Accessor| -> Option<&[u8]> {
                            let view = accessor.view()?;

                            let start = view.offset();
                            let end = start + view.length();

                            let buffer = buffers
                                .get(view.buffer().index())
                                .map(std::ops::Deref::deref)?;

                            Some(&buffer[start..end])
                        };

                        let get_data = |semantic: &gltf::Semantic| -> Option<&[u8]> {
                            primitive.get(semantic).and_then(get_accessor_data)
                        };

                        let get_data_res = |semantic: &gltf::Semantic| -> Result<&[u8]> {
                            get_data(semantic).ok_or_else(|| anyhow!("Missing {semantic:?}"))
                        };

                        let indices_data = primitive
                            .indices()
                            .and_then(get_accessor_data)
                            .ok_or_else(|| anyhow!("Missing indices"))?;
                        let indices = bytemuck::cast_slice::<_, u16>(indices_data)
                            .iter()
                            .map(|&i| i as u32)
                            .collect::<Vec<_>>();

                        let bounding_sphere = {
                            let positions_accessor = primitive
                                .get(&gltf::Semantic::Positions)
                                .ok_or_else(|| anyhow!("Missing positions accessor",))?;

                            let min = serde_json::from_value::<glam::Vec3>(
                                positions_accessor
                                    .min()
                                    .ok_or_else(|| anyhow!("Missing positions accessor min"))?,
                            )?;
                            let max = serde_json::from_value::<glam::Vec3>(
                                positions_accessor
                                    .max()
                                    .ok_or_else(|| anyhow!("Missing positions accessor max"))?,
                            )?;

                            let center = (min + max) / 2.0;
                            let radius = f32::max(
                                (min - center).abs().max_element(),
                                (max - center).abs().max_element(),
                            );

                            (center, radius)
                        };

                        let skin = Option::zip(
                            get_data(&gltf::Semantic::Joints(0)),
                            get_data(&gltf::Semantic::Weights(0)),
                        )
                        .map(|(joints, weights)| {
                            engine.skins.add(&engine.renderer.queue, joints, weights)
                        });

                        let mesh = engine.meshes.add(
                            &engine.renderer.queue,
                            bounding_sphere,
                            get_data_res(&gltf::Semantic::Positions)?,
                            get_data_res(&gltf::Semantic::Normals)?,
                            get_data_res(&gltf::Semantic::Tangents)?,
                            get_data_res(&gltf::Semantic::TexCoords(0))?,
                            bytemuck::cast_slice(&indices),
                            skin,
                        );

                        Ok(mesh)
                    })
                    .collect::<Result<_>>()
            })
            .collect()
    }

    fn build_nodes_transforms(doc: &gltf::Document) -> BTreeMap<usize, glam::Mat4> {
        let mut nodes_transforms: BTreeMap<usize, glam::Mat4> = BTreeMap::new();

        if let Some(scene) = doc.default_scene() {
            fn traverse_nodes_tree<'a, T>(
                nodes: impl Iterator<Item = gltf::Node<'a>>,
                cb: &mut dyn FnMut(&T, &gltf::Node) -> T,
                acc: T,
            ) {
                for node in nodes {
                    let res = cb(&acc, &node);
                    traverse_nodes_tree(node.children(), cb, res);
                }
            }

            traverse_nodes_tree(
                scene.nodes(),
                &mut |parent_transform: &glam::Mat4, node: &gltf::Node| {
                    let local_transform =
                        glam::Mat4::from_cols_array_2d(&node.transform().matrix());
                    let global_transform = *parent_transform * local_transform;

                    nodes_transforms.insert(node.index(), global_transform);
                    global_transform
                },
                glam::Mat4::IDENTITY,
            );
        }

        nodes_transforms
    }

    fn build_skin_animations(
        engine: &mut Engine,
        doc: &gltf::Document,
        nodes_transforms: &BTreeMap<usize, glam::Mat4>,
        buffers: &[gltf::buffer::Data],
    ) -> Vec<HashMap<String, AnimationId>> {
        let animations_samplers: Vec<AnimationSampler> = doc
            .animations()
            .map(|animation| AnimationSampler::new(animation, buffers))
            .collect();

        doc.skins()
            .map(|skin| {
                // Find the node which use this skin
                let mesh_node = doc
                    .nodes()
                    .find(|node| {
                        node.skin()
                            .map(|s| s.index() == skin.index())
                            .unwrap_or(false)
                    })
                    .unwrap();

                let inverse_bind_matrices: Vec<_> = skin
                    .reader(|buffer| buffers.get(buffer.index()).map(std::ops::Deref::deref))
                    .read_inverse_bind_matrices()
                    .unwrap()
                    .map(|arr| glam::Mat4::from_cols_array_2d(&arr))
                    .collect::<Vec<_>>();

                let animation_ids = animations_samplers.iter().map(|sampler| {
                    let (start, end) = sampler.get_time_range();

                    let inv_mesh_transform = nodes_transforms[&mesh_node.index()].inverse();

                    let mut animation: Vec<Vec<glam::Mat4>> = Vec::new();
                    let mut time = start;

                    while time <= end {
                        let animated_nodes_transforms = sampler
                            .get_nodes_transforms(&time, doc.default_scene().unwrap().nodes());

                        // let inv_mesh_transform =
                        //     animated_nodes_transforms[&mesh_node.index()].inverse();
                        let frame: Vec<glam::Mat4> = skin
                            .joints()
                            .zip(&inverse_bind_matrices)
                            .map(|(node, &inverse_bind_matrix)| {
                                let global_joint_transform =
                                    animated_nodes_transforms[&node.index()];
                                inv_mesh_transform * global_joint_transform * inverse_bind_matrix
                            })
                            .collect();

                        animation.push(frame);

                        // time += AnimationsManager::SAMPLE_RATE;
                        time += Duration::from_secs_f32(1.0 / AnimationsManager::SAMPLES_PER_SEC);
                    }

                    engine.animations.add(
                        &engine.renderer.device,
                        &engine.renderer.queue,
                        animation,
                    )
                });

                doc.animations()
                    .map(|animation| animation.name().unwrap_or_default().to_owned())
                    .zip(animation_ids)
                    .collect::<HashMap<_, _>>()
            })
            .collect()
    }
}
