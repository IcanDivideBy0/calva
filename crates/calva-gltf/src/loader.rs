use anyhow::{anyhow, Result};
use calva_renderer::{
    wgpu::{self, util::DeviceExt},
    Renderer, Texture,
};
use std::io::Read;

use crate::{RenderInstances, RenderMaterial, RenderMesh, RenderModel, RenderPrimitive};

macro_rules! label {
    ($s: expr, $obj: expr) => {
        $obj.name()
            .map(|name| format!("{}: {}", $s, name))
            .as_ref()
            .map(String::as_str)
    };
}

pub fn load(renderer: &Renderer, reader: &mut dyn Read) -> Result<RenderModel> {
    let mut gltf_buffer = Vec::new();
    reader.read_to_end(&mut gltf_buffer)?;
    let (doc, buffers, images) = gltf::import_slice(gltf_buffer.as_slice())?;

    // let doc = gltf::Gltf::from_reader(reader)?;

    let get_buffer_data = |accessor: gltf::Accessor| -> Option<&[u8]> {
        let view = accessor.view()?;

        let start = view.offset();
        let end = start + view.length();

        let buffer = buffers.get(view.buffer().index())?;

        Some(&buffer[start..end])
    };

    let device = &renderer.device;

    let meshes = doc
        .meshes()
        .map(|mesh| {
            let primitives = mesh
                .primitives()
                .map(|primitive| -> Result<RenderPrimitive> {
                    let positions_buffer = {
                        let accessor = primitive
                            .get(&gltf::Semantic::Positions)
                            .ok_or(anyhow!("Missing positions accessor"))?;

                        let contents =
                            get_buffer_data(accessor).ok_or(anyhow!("Missing positions buffer"))?;

                        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: label!("Positions buffer", mesh),
                            contents,
                            usage: wgpu::BufferUsages::VERTEX,
                        })
                    };

                    let normals_buffer = {
                        let accessor = primitive
                            .get(&gltf::Semantic::Normals)
                            .ok_or(anyhow!("Missing normals accessor"))?;

                        let contents =
                            get_buffer_data(accessor).ok_or(anyhow!("Missing normals buffer"))?;

                        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: label!("Normals buffer", mesh),
                            contents,
                            usage: wgpu::BufferUsages::VERTEX,
                        })
                    };

                    let tangents_buffer = {
                        let accessor = primitive
                            .get(&gltf::Semantic::Tangents)
                            .ok_or(anyhow!("Missing tangents accessor"))?;

                        let contents =
                            get_buffer_data(accessor).ok_or(anyhow!("Missing tangents buffer"))?;

                        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: label!("Tangents buffer", mesh),
                            contents,
                            usage: wgpu::BufferUsages::VERTEX,
                        })
                    };

                    let tex_coords_0_buffer = {
                        let accessor = primitive
                            .get(&gltf::Semantic::TexCoords(0))
                            .ok_or(anyhow!("Missing texcoords0 accessor"))?;

                        let contents = get_buffer_data(accessor)
                            .ok_or(anyhow!("Missing texcoords0 buffer"))?;

                        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: label!("Texcoords0 buffer", mesh),
                            contents,
                            usage: wgpu::BufferUsages::VERTEX,
                        })
                    };

                    let (indices_buffer, num_elements) = {
                        let accessor = primitive
                            .indices()
                            .ok_or(anyhow!("Missing indices accessor"))?;
                        let num_elements = accessor.count() as u32;

                        let contents =
                            get_buffer_data(accessor).ok_or(anyhow!("Missing indices buffer"))?;

                        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: label!("Indices buffer", mesh),
                            contents,
                            usage: wgpu::BufferUsages::INDEX,
                        });

                        (buffer, num_elements)
                    };

                    Ok(RenderPrimitive {
                        positions_buffer,
                        normals_buffer,
                        tangents_buffer,
                        tex_coords_0_buffer,
                        indices_buffer,
                        num_elements,
                        material: primitive
                            .material()
                            .index()
                            .ok_or(anyhow!("Missing material"))?,
                    })
                })
                .collect::<Result<_>>()?;

            let instances = RenderInstances::new(
                &device,
                vec![(glam::Vec3::default(), glam::Quat::default())],
            );

            Ok(RenderMesh {
                primitives,
                instances,
            })
        })
        .collect::<Result<_>>()?;

    let materials = doc
        .materials()
        .map(|material| {
            let make_texture = |texture: gltf::Texture,
                                format: wgpu::TextureFormat,
                                label: Option<&str>|
             -> Result<Texture> {
                let image_index = texture.source().index();

                let image_data = images
                    .get(image_index)
                    .ok_or(anyhow!("Missing image data"))?;

                // 3 chanels texture formats not supported
                // https://github.com/gpuweb/gpuweb/issues/66
                let pixels = if image_data.format == gltf::image::Format::R8G8B8 {
                    let mut v =
                        Vec::with_capacity((image_data.width * image_data.height * 4) as usize);
                    for pixel in image_data.pixels.chunks_exact(3) {
                        v.extend_from_slice(pixel);
                        v.push(0);
                    }

                    v
                } else {
                    image_data.pixels.clone()
                };

                Ok(Texture::new(
                    &renderer.device,
                    &renderer.queue,
                    wgpu::Extent3d {
                        width: image_data.width,
                        height: image_data.height,
                        depth_or_array_layers: 1,
                    },
                    format,
                    &pixels,
                    label,
                ))
            };

            let base_color_texture = make_texture(
                material
                    .pbr_metallic_roughness()
                    .base_color_texture()
                    .ok_or(anyhow!("Missing base color texture"))?
                    .texture(),
                wgpu::TextureFormat::Rgba8UnormSrgb,
                label!("Base color texture", material),
            )?;

            let normal_texture = make_texture(
                material
                    .normal_texture()
                    .ok_or(anyhow!("Missing normal texture"))?
                    .texture(),
                wgpu::TextureFormat::Rgba8Unorm,
                label!("Normal texture", material),
            )?;

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler {
                                comparison: false,
                                filtering: true,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler {
                                comparison: false,
                                filtering: true,
                            },
                            count: None,
                        },
                    ],
                    label: label!("Bind group layout", material),
                });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&base_color_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&base_color_texture.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&normal_texture.sampler),
                    },
                ],
                label: label!("Bind group", material),
            });

            let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: label!("Material shader", material),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/model.wgsl").into()),
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: label!("Material render pipeline layout", material),
                bind_group_layouts: &[&renderer.camera.bind_group_layout, &bind_group_layout],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: label!("Material render pipeline", material),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "main",
                    buffers: &[
                        RenderInstances::DESC,
                        // Positions
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 3) as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![7 => Float32x3],
                        },
                        // Normals
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 3) as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![8 => Float32x3],
                        },
                        // Tangents
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 4) as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![9 => Float32x4],
                        },
                        // UV
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 2) as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![10 => Float32x2],
                        },
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "main",
                    targets: &Renderer::RENDER_TARGETS,
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    clamp_depth: false,
                    // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(Renderer::DEPTH_STENCIL),
                multisample: Renderer::MULTISAMPLE,
            });

            Ok(RenderMaterial {
                pipeline,
                bind_group,
            })
        })
        .collect::<Result<_>>()?;

    Ok(RenderModel { meshes, materials })
}
