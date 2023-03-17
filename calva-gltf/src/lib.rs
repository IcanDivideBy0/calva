#![warn(clippy::all)]

use anyhow::{anyhow, Result};
use renderer::{
    wgpu, AnimationId, AnimationsManager, Engine, Instance, Material, MaterialId, MeshId,
    PointLight, Renderer, TextureId,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::time::Duration;

mod animation;
use animation::*;

pub struct GltfModel {
    pub doc: gltf::Document,

    meshes_instances: Vec<Vec<Instance>>,
    animations: HashMap<String, AnimationId>,
}

impl GltfModel {
    pub fn from_path(renderer: &Renderer, engine: &mut Engine, path: &str) -> Result<Self> {
        Self::from_reader(renderer, engine, &mut std::fs::File::open(path)?)
    }

    pub fn from_reader(
        renderer: &Renderer,
        engine: &mut Engine,
        reader: &mut dyn Read,
    ) -> Result<Self> {
        let mut gltf_buffer = Vec::new();
        reader.read_to_end(&mut gltf_buffer)?;

        let (doc, buffers, images) = gltf::import_slice(&gltf_buffer)?;

        Self::new(renderer, engine, doc, &buffers, &images)
    }

    pub fn new(
        renderer: &Renderer,
        engine: &mut Engine,
        doc: gltf::Document,
        buffers: &[gltf::buffer::Data],
        images: &[gltf::image::Data],
    ) -> Result<Self> {
        let textures = Self::build_textures(renderer, engine, &doc, images)?;

        let materials = Self::build_materials(renderer, engine, &doc, &textures)?;

        let meshes = Self::build_meshes(renderer, engine, &doc, buffers)?;

        let skins_animations = Self::build_skin_animations(renderer, engine, &doc, buffers);

        let meshes_instances = doc
            .meshes()
            .zip(&meshes)
            .map(|(mesh, meshes_ids)| {
                mesh.primitives()
                    .zip(meshes_ids)
                    .map(|(primitive, &mesh_id)| {
                        let material_id = primitive
                            .material()
                            .index()
                            .and_then(|index| materials.get(index).copied())
                            .unwrap_or_default();

                        Instance {
                            mesh: mesh_id,
                            material: material_id,
                            ..Default::default()
                        }
                    })
                    .collect()
            })
            .collect();

        Ok(Self {
            doc,
            meshes_instances,
            animations: skins_animations.get(0).cloned().unwrap_or_default(),
        })
    }

    fn build_textures(
        renderer: &Renderer,
        engine: &mut Engine,
        doc: &gltf::Document,
        images: &[gltf::image::Data],
    ) -> Result<Vec<TextureId>> {
        let textures = doc
            .images()
            .map(|image| {
                let image_data = images
                    .get(image.index())
                    .ok_or_else(|| anyhow!("Invalid image index"))?;

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
                    label: image.name(),
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

                engine.textures.generate_mipmaps(
                    &renderer.device,
                    &renderer.queue,
                    &texture,
                    &desc,
                )?;

                Ok(engine
                    .textures
                    .add(&renderer.device, texture.create_view(&Default::default())))
            })
            .collect::<Result<Vec<_>>>()?;

        doc.textures()
            .map(|texture| {
                textures
                    .get(texture.source().index())
                    .copied()
                    .ok_or_else(|| anyhow!("Invalid texture image index"))
            })
            .collect()
    }

    fn build_materials(
        renderer: &Renderer,
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

                let emissive = material
                    .emissive_texture()
                    .and_then(|t| textures.get(t.texture().index()).copied())
                    .unwrap_or_default();

                Ok(engine.materials.add(
                    &renderer.queue,
                    Material {
                        albedo,
                        normal,
                        metallic_roughness,
                        emissive,
                    },
                ))
            })
            .collect()
    }

    fn build_meshes(
        renderer: &Renderer,
        engine: &mut Engine,
        doc: &gltf::Document,
        buffers: &[gltf::buffer::Data],
    ) -> Result<Vec<Vec<MeshId>>> {
        doc.meshes()
            .map(|mesh| {
                let mesh_name = mesh.name().unwrap_or("?");

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
                            get_data(semantic)
                                .ok_or_else(|| anyhow!("Mesh [{mesh_name}] missing [{semantic:?}]"))
                        };

                        let indices_data = primitive
                            .indices()
                            .and_then(get_accessor_data)
                            .ok_or_else(|| anyhow!("Mesh [{mesh_name}] missing indices"))?;
                        let indices = bytemuck::cast_slice::<_, u16>(indices_data)
                            .iter()
                            .map(|&i| i as u32)
                            .collect::<Vec<_>>();

                        let bounding_sphere = {
                            let positions_accessor =
                                primitive.get(&gltf::Semantic::Positions).ok_or_else(|| {
                                    anyhow!("Mesh [{mesh_name}] Missing positions accessor",)
                                })?;

                            let min = serde_json::from_value::<glam::Vec3>(
                                positions_accessor.min().ok_or_else(|| {
                                    anyhow!("Mesh [{mesh_name}] Missing positions accessor min")
                                })?,
                            )?;
                            let max = serde_json::from_value::<glam::Vec3>(
                                positions_accessor.max().ok_or_else(|| {
                                    anyhow!("Mesh [{mesh_name}] Missing positions accessor max")
                                })?,
                            )?;

                            let center = (min + max) / 2.0;
                            let radius = (max - center).length();

                            (center, radius)
                        };

                        let skin = Option::zip(
                            get_data(&gltf::Semantic::Joints(0)),
                            get_data(&gltf::Semantic::Weights(0)),
                        )
                        .map(|(joints, weights)| {
                            engine.skins.add(&renderer.queue, joints, weights)
                        });

                        let mesh = engine.meshes.add(
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
            .collect()
    }

    fn build_skin_animations(
        renderer: &Renderer,
        engine: &mut Engine,
        doc: &gltf::Document,
        buffers: &[gltf::buffer::Data],
    ) -> Vec<HashMap<String, AnimationId>> {
        let nodes_transforms = {
            let children_nodes = doc
                .nodes()
                .flat_map(|node| node.children().map(|n| n.index()))
                .collect::<HashSet<_>>();

            let root_nodes = doc
                .nodes()
                .filter(|node| !children_nodes.contains(&node.index()));

            let mut transforms: BTreeMap<usize, glam::Mat4> = BTreeMap::new();

            traverse_nodes_tree(
                root_nodes,
                &mut |parent_transform: &glam::Mat4, node: &gltf::Node| {
                    let local_transform =
                        glam::Mat4::from_cols_array_2d(&node.transform().matrix());
                    let global_transform = *parent_transform * local_transform;

                    transforms.insert(node.index(), global_transform);
                    global_transform
                },
                glam::Mat4::IDENTITY,
            );

            transforms
        };

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

                let inv_mesh_transform = nodes_transforms[&mesh_node.index()].inverse();

                let inverse_bind_matrices: Vec<_> = skin
                    .reader(|buffer| buffers.get(buffer.index()).map(std::ops::Deref::deref))
                    .read_inverse_bind_matrices()
                    .unwrap()
                    .map(|arr| glam::Mat4::from_cols_array_2d(&arr))
                    .collect::<Vec<_>>();

                let animation_ids = animations_samplers.iter().map(|sampler| {
                    let (start, end) = sampler.get_time_range();

                    let mut animation: Vec<Vec<glam::Mat4>> = Vec::new();
                    let mut time = start;

                    while time <= end {
                        let animated_nodes_transforms = sampler
                            .get_nodes_transforms(&time, doc.default_scene().unwrap().nodes());

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

                    engine
                        .animations
                        .add(&renderer.device, &renderer.queue, animation)
                });

                doc.animations()
                    .map(|animation| animation.name().unwrap_or_default().to_owned())
                    .zip(animation_ids)
                    .collect::<HashMap<_, _>>()
            })
            .collect()
    }

    fn node_data(
        &self,
        node: gltf::Node,
        transform: glam::Mat4,
        animation: AnimationId,
    ) -> (Vec<Instance>, Vec<PointLight>) {
        let mut instances = vec![];
        let mut point_lights = vec![];

        traverse_nodes_tree(
            std::iter::once(node),
            &mut |parent_transform: &glam::Mat4, node: &gltf::Node| {
                let local_transform = glam::Mat4::from_cols_array_2d(&node.transform().matrix());
                let global_transform = *parent_transform * local_transform;

                if let Some(mesh_instances) = node
                    .mesh()
                    .and_then(|mesh| self.meshes_instances.get(mesh.index()))
                {
                    instances.extend(mesh_instances.iter().map(|&instance| Instance {
                        transform: global_transform,
                        animation: animation.into(),
                        ..instance
                    }))
                }

                use gltf::khr_lights_punctual::Kind;
                if let Some(light) = node.light() {
                    match light.kind() {
                        Kind::Directional => {
                            unimplemented!();
                        }
                        Kind::Point => {
                            let mut color: glam::Vec3 = light.color().into();
                            let intensity = light.intensity(); // Luminous intensity in candela (lm/sr)
                            const LUMINOUS_EFFICACITY: f32 = 683.002; // Photopic luminous efficacy of radiation
                            color *= intensity / LUMINOUS_EFFICACITY;

                            // Would be more correct, but light radius will grow too much and impact
                            // perfs too much. We can do it after radius computation as well, but light
                            // cutoff becomes a bit too obvious.
                            // color *= 4.0 * std::f32::consts::PI;

                            let position = global_transform.transform_point3(glam::Vec3::ZERO);
                            let radius = light.range().unwrap_or_else(|| {
                                // Calculating a light's volume or radius:
                                // https://learnopengl.com/Advanced-Lighting/Deferred-Shading
                                const CONSTANT: f32 = 1.0;
                                const LINEAR: f32 = 0.7;
                                const QUADRATIC: f32 = 1.8;

                                ((LINEAR * LINEAR
                                    - 4.0
                                        * QUADRATIC
                                        * (CONSTANT - (256.0 / 5.0) * color.max_element()))
                                .sqrt()
                                    - LINEAR)
                                    / (2.0 * QUADRATIC)
                            });

                            point_lights.push(PointLight {
                                position,
                                radius,
                                color,
                            });
                        }
                        Kind::Spot { .. } => {
                            unimplemented!();
                        }
                    }
                }

                global_transform
            },
            transform,
        );

        (instances, point_lights)
    }

    pub fn node_instances(
        &self,
        node_name: &str,
        transform: Option<glam::Mat4>,
        animation_name: Option<&str>,
    ) -> Option<(Vec<Instance>, Vec<PointLight>)> {
        let node = self
            .doc
            .nodes()
            .find(|node| node.name() == Some(node_name))?;

        let transform = transform.unwrap_or_default()
            * glam::Mat4::from_cols_array_2d(&node.transform().matrix()).inverse();

        let animation = match animation_name {
            Some(animation_name) => self.animations.get(animation_name).copied()?,
            _ => Default::default(),
        };

        Some(self.node_data(node, transform, animation))
    }

    fn scene_data(
        &self,
        scene: gltf::Scene,
        animation: AnimationId,
        transform: glam::Mat4,
    ) -> (Vec<Instance>, Vec<PointLight>) {
        let mut instances = vec![];
        let mut point_lights = vec![];

        for node in scene.nodes() {
            let (node_instances, node_point_lights) = self.node_data(node, transform, animation);
            instances.extend(node_instances);
            point_lights.extend(node_point_lights);
        }

        (instances, point_lights)
    }

    pub fn scene_instances(
        &self,
        scene_name: Option<&str>,
        animation_name: Option<&str>,
        transform: Option<glam::Mat4>,
    ) -> Option<(Vec<Instance>, Vec<PointLight>)> {
        let scene = if let Some(scene_name) = scene_name {
            self.doc
                .scenes()
                .find(|scene| scene.name() == Some(scene_name))?
        } else {
            self.doc.default_scene()?
        };

        let animation = if let Some(animation_name) = animation_name {
            self.animations.get(animation_name).copied()?
        } else {
            AnimationId::default()
        };

        Some(self.scene_data(scene, animation, transform.unwrap_or_default()))
    }

    pub fn animations(&self) -> impl Iterator<Item = &String> {
        self.animations.keys()
    }
}

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
