use anyhow::{anyhow, Result};
use renderer::{
    wgpu::{self, util::DeviceExt},
    Material, Mesh, MeshInstances, Renderer, Skin, SkinAnimation,
};
use std::collections::HashMap;
use std::io::Read;

mod animation;
mod util;

use animation::*;

macro_rules! label {
    ($s: expr, $obj: expr) => {
        $obj.name()
            .map(|name| format!("{}: {}", $s, name))
            .as_ref()
            .map(String::as_str)
    };
}

fn traverse_nodes<'a>(
    nodes: impl Iterator<Item = gltf::Node<'a>>,
    pwt: glam::Mat4, // parent world transform
    store: &mut HashMap<usize, glam::Mat4>,
) {
    for node in nodes {
        let local_transform = glam::Mat4::from_cols_array_2d(&node.transform().matrix());
        let world_transform = pwt * local_transform;
        store.insert(node.index(), world_transform);
        traverse_nodes(node.children(), world_transform, store);
    }
}

pub struct GltfModel {
    pub meshes: Vec<(Mesh, Option<Skin>, usize, usize)>,
    pub materials: Vec<Material>,
    pub instances: Vec<MeshInstances>,
    pub animations: Vec<SkinAnimation>,
}

impl GltfModel {
    pub fn new(renderer: &Renderer, reader: &mut dyn Read) -> Result<Self> {
        let Renderer { device, queue, .. } = renderer;

        let mut gltf_buffer = Vec::new();
        reader.read_to_end(&mut gltf_buffer)?;

        let (doc, buffers, images) = gltf::import_slice(gltf_buffer.as_slice())?;

        let get_buffer_data = util::buffer_reader(&buffers);
        let mut get_accessor_data = |accessor: gltf::Accessor| -> Option<&[u8]> {
            let view = accessor.view()?;

            let start = view.offset();
            let end = start + view.length();

            let buffer = get_buffer_data(view.buffer())?;

            Some(&buffer[start..end])
        };
        let mut get_image_data = util::image_reader(&images);
        let mut make_texture = util::texture_builder(device, queue);

        let materials: Vec<Material> = doc
            .materials()
            .map(|material| {
                let albedo = make_texture(
                    label!("Albedo texture", material),
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                    material
                        .pbr_metallic_roughness()
                        .base_color_texture()
                        .map(|t| t.texture())
                        .and_then(&mut get_image_data)
                        .ok_or_else(|| anyhow!("Missing base color texture"))?,
                )
                .create_view(&wgpu::TextureViewDescriptor::default());

                let normal = make_texture(
                    label!("Normal texture", material),
                    wgpu::TextureFormat::Rgba8Unorm,
                    material
                        .normal_texture()
                        .map(|t| t.texture())
                        .and_then(&mut get_image_data)
                        .unwrap_or_else(|| {
                            let mut buf = image::ImageBuffer::new(1, 1);
                            buf.put_pixel(0, 0, image::Rgba::from([0, 0, 0, 0]));
                            image::DynamicImage::ImageRgba8(buf)
                        }),
                )
                .create_view(&wgpu::TextureViewDescriptor::default());

                let metallic_roughness = make_texture(
                    label!("Metallic roughness texture", material),
                    wgpu::TextureFormat::Rgba8Unorm,
                    material
                        .pbr_metallic_roughness()
                        .metallic_roughness_texture()
                        .map(|t| t.texture())
                        .and_then(&mut get_image_data)
                        .unwrap_or_else(|| {
                            let mut buf = image::ImageBuffer::new(1, 1);
                            buf.put_pixel(0, 0, image::Rgba::from([0, 0xFF, 0xFF, 0]));
                            image::DynamicImage::ImageRgba8(buf)
                        }),
                )
                .create_view(&wgpu::TextureViewDescriptor::default());

                Ok(Material::new(device, &albedo, &normal, &metallic_roughness))
            })
            .collect::<Result<_>>()?;

        let mut nodes_transforms = HashMap::new();
        for scene in doc.scenes() {
            traverse_nodes(scene.nodes(), glam::Mat4::IDENTITY, &mut nodes_transforms);
        }

        let mut meshes_transforms = vec![Vec::new(); doc.meshes().len()];
        for node in doc.nodes() {
            if let Some(mesh) = node.mesh() {
                meshes_transforms[mesh.index()].push(nodes_transforms[&node.index()])
            }
        }

        let mut instances = doc
            .meshes()
            .map(|mesh| MeshInstances::new(device, meshes_transforms[mesh.index()].clone()))
            .collect::<Vec<_>>();

        let primitives_count = doc
            .meshes()
            .fold(0, |acc, mesh| acc + mesh.primitives().len());

        let meshes = doc.meshes().try_fold(
            Vec::with_capacity(primitives_count),
            |mut acc, mesh| -> Result<_> {
                for primitive in mesh.primitives() {
                    let vertices = {
                        let accessor = primitive
                            .get(&gltf::Semantic::Positions)
                            .ok_or_else(|| anyhow!("Missing positions accessor"))?;

                        let contents = get_accessor_data(accessor)
                            .ok_or_else(|| anyhow!("Missing positions buffer"))?;
                        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: label!("Positions buffer", mesh),
                            contents,
                            usage: wgpu::BufferUsages::VERTEX,
                        })
                    };

                    let normals = {
                        let accessor = primitive
                            .get(&gltf::Semantic::Normals)
                            .ok_or_else(|| anyhow!("Missing normals accessor"))?;

                        let contents = get_accessor_data(accessor)
                            .ok_or_else(|| anyhow!("Missing normals buffer"))?;
                        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: label!("Normals buffer", mesh),
                            contents,
                            usage: wgpu::BufferUsages::VERTEX,
                        })
                    };

                    let tangents = {
                        let accessor = primitive
                            .get(&gltf::Semantic::Tangents)
                            .ok_or_else(|| anyhow!("Missing tangents accessor"))?;

                        let contents = get_accessor_data(accessor)
                            .ok_or_else(|| anyhow!("Missing tangents buffer"))?;
                        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: label!("Tangents buffer", mesh),
                            contents,
                            usage: wgpu::BufferUsages::VERTEX,
                        })
                    };

                    let uv0 = {
                        let accessor = primitive
                            .get(&gltf::Semantic::TexCoords(0))
                            .ok_or_else(|| anyhow!("Missing texCoords0 accessor"))?;

                        let contents = get_accessor_data(accessor)
                            .ok_or_else(|| anyhow!("Missing texCoords0 buffer"))?;
                        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: label!("TexCoords0 buffer", mesh),
                            contents,
                            usage: wgpu::BufferUsages::VERTEX,
                        })
                    };

                    let (indices, num_elements) = {
                        let accessor = primitive
                            .indices()
                            .ok_or_else(|| anyhow!("Missing indices accessor"))?;
                        let num_elements = accessor.count() as u32;

                        let contents = get_accessor_data(accessor)
                            .ok_or_else(|| anyhow!("Missing indices buffer"))?;
                        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: label!("Indices buffer", mesh),
                            contents,
                            usage: wgpu::BufferUsages::INDEX,
                        });
                        (buffer, num_elements)
                    };

                    let skin = {
                        let joint_indices = primitive
                            .get(&gltf::Semantic::Joints(0))
                            .and_then(&mut get_accessor_data)
                            .map(|contents| {
                                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: label!("Joints buffer", mesh),
                                    contents,
                                    usage: wgpu::BufferUsages::VERTEX,
                                })
                            });

                        let joint_weights = primitive
                            .get(&gltf::Semantic::Weights(0))
                            .and_then(&mut get_accessor_data)
                            .map(|contents| {
                                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: label!("Weights buffer", mesh),
                                    contents,
                                    usage: wgpu::BufferUsages::VERTEX,
                                })
                            });

                        joint_indices
                            .zip(joint_weights)
                            .map(|(joint_indices, joint_weights)| Skin {
                                joint_indices,
                                joint_weights,
                            })
                    };

                    acc.push((
                        Mesh {
                            vertices,
                            normals,
                            tangents,
                            uv0,
                            indices,
                            num_elements,
                        },
                        skin,
                        primitive
                            .material()
                            .index()
                            .ok_or_else(|| anyhow!("Invalid material"))?,
                        mesh.index(),
                    ));
                }

                Ok(acc)
            },
        )?;

        let animations: HashMap<String, Animation> = doc
            .animations()
            .map(|animation| {
                (
                    animation.name().unwrap().to_owned(),
                    Animation::new(animation, &buffers),
                )
            })
            .collect();

        let animations_data = doc
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
                    .reader(get_buffer_data.clone())
                    .read_inverse_bind_matrices()
                    .unwrap()
                    .map(|a| glam::Mat4::from_cols_array_2d(&a))
                    .collect::<Vec<_>>();

                // for (_, samplers) in &animations {}
                let animation = &animations["run"];

                let (mut start, end) = animation.get_time_range();
                let interval = std::time::Duration::from_secs_f32(1.0 / 60.0);
                let mut texture_data = vec![];
                while start < end {
                    let animated_nodes_transforms = animation
                        .get_nodes_transforms(&start, doc.default_scene().unwrap().nodes());

                    let mesh_transform = animated_nodes_transforms[&mesh_node.index()];
                    let inv_mesh_transform = mesh_transform.inverse();

                    let mut frame_data = skin
                        .joints()
                        .zip(&inverse_bind_matrices)
                        .map(|(node, &inverse_bind_matrix)| {
                            let global_joint_transform = animated_nodes_transforms[&node.index()];
                            inv_mesh_transform * global_joint_transform * inverse_bind_matrix
                        })
                        .collect::<Vec<_>>();

                    texture_data.append(&mut frame_data);

                    start += interval;
                }

                let animated_nodes_transforms = animation.get_nodes_transforms(
                    &std::time::Duration::from_secs_f32(0.0),
                    doc.default_scene().unwrap().nodes(),
                );

                let mesh_transform = animated_nodes_transforms[&mesh_node.index()];
                let inv_mesh_transform = mesh_transform.inverse();

                instances[mesh_node.mesh().unwrap().index()].transforms[0] = mesh_transform;

                skin.joints()
                    .zip(&inverse_bind_matrices)
                    .map(|(node, &inverse_bind_matrix)| {
                        let global_joint_transform = animated_nodes_transforms[&node.index()];
                        inv_mesh_transform * global_joint_transform * inverse_bind_matrix
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        Ok(Self {
            meshes,
            materials,
            instances,
            animations: vec![SkinAnimation::new(device, animations_data)],
        })
    }
}
