use anyhow::{anyhow, Result};
use renderer::{
    util::mipmap::MipmapGenerator,
    wgpu::{self, util::DeviceExt},
    Material, Mesh, MeshInstances, Renderer,
};
use std::collections::HashMap;
use std::io::Read;

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
    parent: glam::Mat4,
    store: &mut HashMap<usize, glam::Mat4>,
) {
    for node in nodes {
        let transform = parent * glam::Mat4::from_cols_array_2d(&node.transform().matrix());
        store.insert(node.index(), transform);
        traverse_nodes(node.children(), transform, store);
    }
}

pub struct GltfModel {
    pub meshes: Vec<(Mesh, usize, usize)>,
    pub materials: Vec<Material>,
    pub instances: Vec<MeshInstances>,
}

impl GltfModel {
    pub fn new(renderer: &Renderer, reader: &mut dyn Read) -> Result<Self> {
        let Renderer { device, queue, .. } = renderer;

        let mut gltf_buffer = Vec::new();
        reader.read_to_end(&mut gltf_buffer)?;

        let (doc, buffers, images) = gltf::import_slice(gltf_buffer.as_slice())?;

        let get_buffer_data = |accessor: gltf::Accessor| -> Option<&[u8]> {
            let view = accessor.view()?;

            let start = view.offset();
            let end = start + view.length();

            let buffer = buffers.get(view.buffer().index())?;

            Some(&buffer[start..end])
        };

        let get_image_data = |texture: gltf::Texture| -> Option<_> {
            let image_index = texture.source().index();

            let image_data = images.get(image_index)?;

            // 3 channels texture formats are not supported by WebGPU
            // https://github.com/gpuweb/gpuweb/issues/66
            if image_data.format == gltf::image::Format::R8G8B8 {
                let buf = image::ImageBuffer::from_raw(
                    image_data.width,
                    image_data.height,
                    image_data.pixels.clone(),
                )?;

                Some(image::DynamicImage::ImageRgb8(buf))
            } else {
                let buf = image::ImageBuffer::from_raw(
                    image_data.width,
                    image_data.height,
                    image_data.pixels.clone(),
                )?;

                Some(image::DynamicImage::ImageRgba8(buf))
            }
        };

        let mipmap_generator = MipmapGenerator::new(device);

        let make_texture =
            |label: Option<&str>, format: wgpu::TextureFormat, image: image::DynamicImage| {
                let buf = image.to_rgba8();

                let size = wgpu::Extent3d {
                    width: buf.width(),
                    height: buf.height(),
                    depth_or_array_layers: 1,
                };

                let desc = wgpu::TextureDescriptor {
                    label,
                    size,
                    mip_level_count: size.max_mips(),
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::COPY_DST,
                };

                let texture = device.create_texture(&desc);

                queue.write_texture(
                    texture.as_image_copy(),
                    &buf.to_vec(),
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: std::num::NonZeroU32::new(4 * size.width),
                        rows_per_image: None,
                    },
                    size,
                );

                mipmap_generator.generate_mipmaps(device, queue, &texture, &desc);

                texture
            };

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
                        .and_then(get_image_data)
                        .ok_or_else(|| anyhow!("Missing base color texture"))?,
                )
                .create_view(&wgpu::TextureViewDescriptor::default());

                let normal = make_texture(
                    label!("Normal texture", material),
                    wgpu::TextureFormat::Rgba8Unorm,
                    material
                        .normal_texture()
                        .map(|t| t.texture())
                        .and_then(get_image_data)
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
                        .and_then(get_image_data)
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

        let mut transforms = HashMap::new();
        for scene in doc.scenes() {
            traverse_nodes(scene.nodes(), glam::Mat4::IDENTITY, &mut transforms);
        }

        let mut meshes_transforms = vec![Vec::new(); doc.meshes().len()];
        for node in doc.nodes() {
            if let Some(mesh) = node.mesh() {
                meshes_transforms[mesh.index()].push(transforms[&node.index()])
            }
        }

        let instances = doc
            .meshes()
            .map(|mesh| MeshInstances::new(device, meshes_transforms[mesh.index()].clone()))
            .collect();

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

                        let contents = get_buffer_data(accessor)
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

                        let contents = get_buffer_data(accessor)
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

                        let contents = get_buffer_data(accessor)
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

                        let contents = get_buffer_data(accessor)
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

                        let contents = get_buffer_data(accessor)
                            .ok_or_else(|| anyhow!("Missing indices buffer"))?;
                        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: label!("Indices buffer", mesh),
                            contents,
                            usage: wgpu::BufferUsages::INDEX,
                        });
                        (buffer, num_elements)
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

        Ok(Self {
            meshes,
            materials,
            instances,
        })
    }
}
