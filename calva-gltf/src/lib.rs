use anyhow::{anyhow, Result};
use renderer::{
    wgpu, AnimationId, AnimationState, AnimationsManager, GeometryPass, Material, MaterialId,
    MeshId, MeshInstance, Renderer, TextureId,
};
use std::collections::HashMap;
use std::io::Read;
use std::time::Duration;

mod animation;
use animation::*;

pub struct GltfModel {
    pub instances: Vec<MeshInstance>,
    pub animations: HashMap<String, AnimationId>,
}

impl GltfModel {
    pub fn from_reader(
        renderer: &mut Renderer,
        geometry: &mut GeometryPass,
        reader: &mut dyn Read,
    ) -> Result<Self> {
        let mut gltf_buffer = Vec::new();
        reader.read_to_end(&mut gltf_buffer)?;

        let (doc, buffers, images) = gltf::import_slice(&gltf_buffer)?;

        Self::new(renderer, geometry, &doc, &buffers, &images)
    }

    pub fn new(
        renderer: &mut Renderer,
        geometry: &mut GeometryPass,
        doc: &gltf::Document,
        buffers: &[gltf::buffer::Data],
        images: &[gltf::image::Data],
    ) -> Result<Self> {
        let textures: Vec<TextureId> = doc
            .textures()
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

                let texture = renderer.device.create_texture(&desc);

                renderer.queue.write_texture(
                    texture.as_image_copy(),
                    &buf.to_rgba8(),
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: std::num::NonZeroU32::new(4 * size.width),
                        rows_per_image: None,
                    },
                    size,
                );

                geometry.textures.generate_mipmaps(
                    &renderer.device,
                    &renderer.queue,
                    &texture,
                    &desc,
                )?;

                Ok(geometry
                    .textures
                    .add(&renderer.device, texture.create_view(&Default::default())))
            })
            .collect::<Result<_>>()?;

        let materials: Vec<MaterialId> = doc
            .materials()
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

                Ok(geometry.materials.add(
                    &renderer.queue,
                    Material {
                        albedo,
                        normal,
                        metallic_roughness,
                    },
                ))
            })
            .collect::<Result<_>>()?;

        let meshes: Vec<Vec<MeshId>> = doc
            .meshes()
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
                            geometry.skins.add(&renderer.queue, joints, weights)
                        });

                        let mesh = geometry.meshes.add(
                            &renderer.queue,
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
            .collect::<Result<_>>()?;

        let mut nodes_transforms: HashMap<usize, glam::Mat4> = HashMap::new();
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

        let animations_samplers: Vec<AnimationSampler> = doc
            .animations()
            .map(|animation| AnimationSampler::new(animation, buffers))
            .collect();

        let skins_animations: Vec<HashMap<String, AnimationId>> = doc
            .skins()
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

                    geometry
                        .animations
                        .add(&renderer.device, &renderer.queue, animation)
                });

                doc.animations()
                    .map(|animation| animation.name().unwrap_or_default().to_owned())
                    .zip(animation_ids)
                    .collect::<HashMap<_, _>>()
            })
            .collect();

        let instances: Vec<MeshInstance> = doc
            .nodes()
            .filter_map(|node| {
                let mesh = node.mesh()?;
                let mesh_data = meshes.get(mesh.index())?;
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
                    .zip(mesh_data)
                    .map(|(primitive, &mesh_id)| {
                        let material_id = primitive
                            .material()
                            .index()
                            .and_then(|index| materials.get(index).copied())
                            .unwrap_or_default();

                        MeshInstance {
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

        Ok(Self {
            instances,
            animations: skins_animations.get(0).cloned().unwrap_or_default(),
        })
    }
}
