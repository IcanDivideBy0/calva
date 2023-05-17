#![warn(clippy::all)]

use anyhow::{anyhow, Result};
use renderer::{
    wgpu, AnimationId, AnimationsManager, Engine, Instance, Material, MaterialId, MaterialsManager,
    MeshId, MeshesManager, PointLight, Renderer, SkinsManager, TextureId, TexturesManager,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::Read,
    time::Duration,
};

mod animation;
use animation::*;

pub struct GltfModel {
    pub doc: gltf::Document,

    meshes_instances: Vec<Vec<Instance>>,
    pub animations: HashMap<String, AnimationId>,
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
                        bytes_per_row: Some(4 * size.width),
                        rows_per_image: None,
                    },
                    size,
                );

                engine
                    .ressources
                    .get::<TexturesManager>()
                    .get()
                    .generate_mipmaps(&renderer.device, &renderer.queue, &texture, &desc)?;

                Ok(engine
                    .ressources
                    .get::<TexturesManager>()
                    .get_mut()
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

                Ok(engine.ressources.get::<MaterialsManager>().get().add(
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
                        let get_buffer_data = |buffer: gltf::Buffer| -> Option<&[u8]> {
                            buffers.get(buffer.index()).map(std::ops::Deref::deref)
                        };

                        let get_accessor_data = |accessor: gltf::Accessor| -> Option<&[u8]> {
                            let view = accessor.view()?;

                            let start = view.offset();
                            let end = start + view.length();

                            let buffer = get_buffer_data(view.buffer())?;

                            Some(&buffer[start..end])
                        };

                        let get_data = |semantic: &gltf::Semantic| -> Option<&[u8]> {
                            primitive.get(semantic).and_then(get_accessor_data)
                        };

                        let get_data_res = |semantic: &gltf::Semantic| -> Result<&[u8]> {
                            get_data(semantic)
                                .ok_or_else(|| anyhow!("Mesh [{mesh_name}] missing [{semantic:?}]"))
                        };

                        let indices = primitive
                            .reader(get_buffer_data)
                            .read_indices()
                            .unwrap()
                            .into_u32()
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
                            engine.ressources.get::<SkinsManager>().get_mut().add(
                                &renderer.queue,
                                joints,
                                weights,
                            )
                        });

                        let mesh = engine.ressources.get::<MeshesManager>().get().add(
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
                &mut |parent_transform, node| {
                    let transform = *parent_transform
                        * glam::Mat4::from_cols_array_2d(&node.transform().matrix());

                    transforms.insert(node.index(), transform);

                    Some(transform)
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

                    engine.ressources.get::<AnimationsManager>().get_mut().add(
                        &renderer.device,
                        &renderer.queue,
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

    fn nodes_data<'a>(
        &self,
        nodes: impl Iterator<Item = gltf::Node<'a>>,
        transform: glam::Mat4,
        animation: Option<AnimationId>,
    ) -> (Vec<Instance>, Vec<PointLight>) {
        let mut instances = vec![];
        let mut point_lights = vec![];

        traverse_nodes_tree(
            nodes,
            &mut |parent_transform, node| {
                let transform =
                    *parent_transform * glam::Mat4::from_cols_array_2d(&node.transform().matrix());

                let mesh_instances = node
                    .mesh()
                    .and_then(|mesh| self.meshes_instances.get(mesh.index()));
                if let Some(mesh_instances) = mesh_instances {
                    instances.extend(mesh_instances.iter().map(|&instance| Instance {
                        transform,
                        animation: animation.unwrap_or_default().into(),
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
                            let position = transform.transform_point3(glam::Vec3::ZERO);

                            const WATTS_TO_LUMENS: f32 = 683.0;
                            // Luminous intensity in candela (lm/sr) ; multiplied by 4π to get luminous power (lumens) ; converted to watts
                            let intensity =
                                light.intensity() * (4.0 * std::f32::consts::PI) / WATTS_TO_LUMENS;

                            let color = glam::Vec3::from(light.color()) * intensity;

                            let radius = light.range().unwrap_or_else(|| {
                                const ATTENUATION_MAX: f32 = 1.0 - (5.0 / 256.0);
                                (color.max_element() * ATTENUATION_MAX).sqrt()
                            });

                            // There must be an error in blender export, removing the 4π factor will give the exact
                            // same result as blender renders when using the same exposure algorithm, but we also
                            // need to keep it for radius computation to get a somewhat similar range :/
                            let color = color / (4.0 * std::f32::consts::PI);

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

                Some(transform)
            },
            transform,
        );

        (instances, point_lights)
    }

    pub fn node_instances(
        &self,
        node: gltf::Node,
        transform: Option<glam::Mat4>,
        animation: Option<AnimationId>,
    ) -> (Vec<Instance>, Vec<PointLight>) {
        let transform = transform.unwrap_or_default()
            * glam::Mat4::from_cols_array_2d(&node.transform().matrix()).inverse();

        self.nodes_data(std::iter::once(node), transform, animation)
    }

    fn scene_data(
        &self,
        scene: gltf::Scene,
        transform: glam::Mat4,
        animation: Option<AnimationId>,
    ) -> (Vec<Instance>, Vec<PointLight>) {
        self.nodes_data(scene.nodes(), transform, animation)
    }

    pub fn scene_instances(
        &self,
        scene_name: Option<&str>,
        transform: Option<glam::Mat4>,
        animation: Option<AnimationId>,
    ) -> Option<(Vec<Instance>, Vec<PointLight>)> {
        let scene = if let Some(scene_name) = scene_name {
            self.doc
                .scenes()
                .find(|scene| scene.name() == Some(scene_name))?
        } else {
            self.doc.default_scene()?
        };

        Some(self.scene_data(scene, transform.unwrap_or_default(), animation))
    }

    pub fn get_node(&self, name: &str) -> Option<gltf::Node> {
        self.doc.nodes().find(|node| node.name() == Some(name))
    }
    pub fn get_animation(&self, name: &str) -> Option<AnimationId> {
        self.animations.get(name).copied()
    }
}

pub fn traverse_nodes_tree<'a, T>(
    nodes: impl Iterator<Item = gltf::Node<'a>>,
    visitor: &mut dyn FnMut(&T, &gltf::Node) -> Option<T>,
    acc: T,
) {
    for node in nodes {
        if let Some(res) = visitor(&acc, &node) {
            traverse_nodes_tree(node.children(), visitor, res);
        }
    }
}
