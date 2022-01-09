use crate::{
    Instance, Material, Mesh, MeshInstance, MeshInstances, RenderContext, Renderer, Skin,
    SkinAnimationInstance, SkinAnimationInstances, SkinAnimations,
};

pub type DrawCallArgs<'a> = (
    &'a MeshInstances,
    &'a Mesh,
    &'a Material,
    Option<&'a Skin>,
    Option<&'a SkinAnimationInstances>,
    Option<&'a SkinAnimations>,
);

pub struct Geometry {
    pub albedo_metallic: wgpu::TextureView,
    pub normal_roughness: wgpu::TextureView,
    pub depth: wgpu::TextureView,

    size: wgpu::Extent3d,
    depth_texture: wgpu::Texture,

    simple_mesh_pipeline: wgpu::RenderPipeline,
    skinned_mesh_pipeline: wgpu::RenderPipeline,
}

impl Geometry {
    const ALBEDO_METALLIC_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
    const NORMAL_ROUGHNESS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    const RENDER_TARGETS: &'static [wgpu::ColorTargetState] = &[
        wgpu::ColorTargetState {
            format: Self::ALBEDO_METALLIC_FORMAT,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        },
        wgpu::ColorTargetState {
            format: Self::NORMAL_ROUGHNESS_FORMAT,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        },
    ];

    pub fn new(renderer: &Renderer) -> Self {
        let Renderer {
            device,
            surface_config,
            camera,
            ..
        } = renderer;

        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };

        let albedo_metallic = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("GBuffer albedo/metallic texture"),
                size,
                mip_level_count: 1,
                sample_count: Renderer::MULTISAMPLE_STATE.count,
                dimension: wgpu::TextureDimension::D2,
                format: Self::ALBEDO_METALLIC_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let normal_roughness = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("GBuffer normal/roughness texture"),
                size,
                mip_level_count: 1,
                sample_count: Renderer::MULTISAMPLE_STATE.count,
                dimension: wgpu::TextureDimension::D2,
                format: Self::NORMAL_ROUGHNESS_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GBuffer depth texture"),
            size,
            mip_level_count: 1,
            sample_count: Renderer::MULTISAMPLE_STATE.count,
            dimension: wgpu::TextureDimension::D2,
            format: Renderer::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        });

        let depth = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            aspect: wgpu::TextureAspect::DepthOnly,
            ..Default::default()
        });

        let simple_mesh_pipeline = {
            let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("Geometry[simple] shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("shaders/geometry.simple.wgsl").into(),
                ),
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Geometry[simple] render pipeline layout"),
                bind_group_layouts: &[
                    &camera.bind_group_layout,
                    &device.create_bind_group_layout(Material::DESC),
                ],
                push_constant_ranges: &[],
            });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Geometry[simple] render pipeline"),
                layout: Some(&pipeline_layout),
                multiview: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[
                        MeshInstance::LAYOUT,
                        // Positions
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 3) as _,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![8 => Float32x3],
                        },
                        // Normals
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 3) as _,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![9 => Float32x3],
                        },
                        // Tangents
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 4) as _,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![10 => Float32x4],
                        },
                        // UV
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 2) as _,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![11 => Float32x2],
                        },
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: Self::RENDER_TARGETS,
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Renderer::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: Renderer::MULTISAMPLE_STATE,
            })
        };

        let skinned_mesh_pipeline = {
            let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("Geometry[skinned] shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("shaders/geometry.skinned.wgsl").into(),
                ),
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Geometry[skinned] render pipeline layout"),
                bind_group_layouts: &[
                    &camera.bind_group_layout,
                    &device.create_bind_group_layout(Material::DESC),
                    &device.create_bind_group_layout(SkinAnimations::DESC),
                ],
                push_constant_ranges: &[],
            });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Geometry[skinned] render pipeline"),
                layout: Some(&pipeline_layout),
                multiview: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[
                        MeshInstance::LAYOUT,
                        SkinAnimationInstance::LAYOUT,
                        // Positions
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 3) as _,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![8 => Float32x3],
                        },
                        // Normals
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 3) as _,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![9 => Float32x3],
                        },
                        // Tangents
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 4) as _,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![10 => Float32x4],
                        },
                        // UV
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 2) as _,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![11 => Float32x2],
                        },
                        // Joints
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<u32>()) as _,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![12 => Uint32], // Do not use Uint8x4 or it will mess up offsets/strides
                        },
                        // Weights
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 4) as _,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![13 => Float32x4],
                        },
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: Self::RENDER_TARGETS,
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Renderer::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: Renderer::MULTISAMPLE_STATE,
            })
        };

        Self {
            albedo_metallic,
            normal_roughness,
            depth,

            size,
            depth_texture,

            simple_mesh_pipeline,
            skinned_mesh_pipeline,
        }
    }

    pub fn render<'s: 'ctx, 'ctx, 'data: 'ctx>(
        &'s self,
        ctx: &'ctx mut RenderContext,
        cb: impl FnOnce(&mut dyn FnMut(DrawCallArgs<'data>)),
    ) {
        ctx.encoder.push_debug_group("Geometry");

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Geometry render pass"),
            color_attachments: &[
                wgpu::RenderPassColorAttachment {
                    view: &self.albedo_metallic,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                },
                wgpu::RenderPassColorAttachment {
                    view: &self.normal_roughness,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                },
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        cb(
            &mut |(mesh_instances, mesh, material, skin, animation_instances, animation): DrawCallArgs| {
                rpass.set_pipeline(match skin {
                    Some(_) => &self.skinned_mesh_pipeline,
                    None => &self.simple_mesh_pipeline,
                });

                rpass.set_bind_group(0, &ctx.renderer.camera.bind_group, &[]);
                rpass.set_bind_group(1, &material.bind_group, &[]);

                if let Some(animation) = animation {
                    rpass.set_bind_group(2, &animation.bind_group, &[]);
                }

                let mut idx_iter = 0..;
                macro_rules! idx {
                    () => {
                        idx_iter.next().unwrap()
                    };
                }

                rpass.set_vertex_buffer(idx!(), mesh_instances.buffer.slice(..));
                if let Some(animation_instances) = animation_instances {
                    rpass.set_vertex_buffer(idx!(), animation_instances.buffer.slice(..));
                }

                rpass.set_vertex_buffer(idx!(), mesh.vertices.slice(..));
                rpass.set_vertex_buffer(idx!(), mesh.normals.slice(..));
                rpass.set_vertex_buffer(idx!(), mesh.tangents.slice(..));
                rpass.set_vertex_buffer(idx!(), mesh.uv0.slice(..));

                if let Some(skin) = skin {
                    rpass.set_vertex_buffer(idx!(), skin.joint_indices.slice(..));
                    rpass.set_vertex_buffer(idx!(), skin.joint_weights.slice(..));
                }

                rpass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..mesh.num_elements, 0, 0..mesh_instances.count());
            },
        );

        drop(rpass);

        ctx.encoder.copy_texture_to_texture(
            self.depth_texture.as_image_copy(),
            ctx.renderer.depth_stencil_texture.as_image_copy(),
            self.size,
        );

        ctx.encoder.pop_debug_group();
    }
}
