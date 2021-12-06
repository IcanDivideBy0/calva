use wgpu::util::DeviceExt;

use crate::RenderContext;
use crate::Renderer;

pub struct SsaoPass {
    pub output: wgpu::TextureView,

    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    blur: SsaoBlur,
}

impl SsaoPass {
    const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

    pub fn new(renderer: &Renderer, normal: &wgpu::TextureView, depth: &wgpu::TextureView) -> Self {
        let output = renderer
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("SsaoPass output"),
                size: wgpu::Extent3d {
                    width: renderer.surface_config.width,
                    height: renderer.surface_config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::OUTPUT_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let random_data_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("SsaoPass random data buffer"),
                    contents: bytemuck::cast_slice(&[SsaoUniform::new()]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

        let bind_group_layout =
            renderer
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("SsaoPass bind group layout"),
                    entries: &[
                        // random data
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // depth
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: Renderer::MULTISAMPLE_STATE.count > 1,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Depth,
                            },
                            count: None,
                        },
                        // normals
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: Renderer::MULTISAMPLE_STATE.count > 1,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            },
                            count: None,
                        },
                    ],
                });

        let bind_group = renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("SsaoPass bind group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: random_data_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(depth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(normal),
                    },
                ],
            });

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("SsaoPass pipeline layout"),
                    bind_group_layouts: &[
                        &renderer.config.bind_group_layout,
                        &renderer.camera.bind_group_layout,
                        &bind_group_layout,
                    ],
                    push_constant_ranges: &[],
                });

        let shader = renderer
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("SsaoPass shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ssao.wgsl").into()),
            });

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("SsaoPass pipeline"),
                layout: Some(&pipeline_layout),
                multiview: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "main",
                    targets: &[wgpu::ColorTargetState {
                        format: Self::OUTPUT_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            });

        let blur = SsaoBlur::new(renderer, &output);

        Self {
            output,

            bind_group,
            pipeline,
            blur,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        {
            let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SsaoPass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &self.output,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &ctx.renderer.config.bind_group, &[]);
            rpass.set_bind_group(1, &ctx.renderer.camera.bind_group, &[]);
            rpass.set_bind_group(2, &self.bind_group, &[]);

            rpass.draw(0..3, 0..1);
        }

        self.blur.render(ctx, &self.output)
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SsaoUniform {
    samples: [glam::Vec2; SsaoUniform::SAMPLES_COUNT],
    noise: [glam::Vec2; 16],
}

impl SsaoUniform {
    const SAMPLES_COUNT: usize = 32;

    fn new() -> Self {
        let samples: [_; Self::SAMPLES_COUNT] = (0..Self::SAMPLES_COUNT)
            .map(|i| {
                let sample = glam::vec2(
                    rand::random::<f32>() * 2.0 - 1.0,
                    rand::random::<f32>() * 2.0 - 1.0,
                );

                let scale = i as f32 / Self::SAMPLES_COUNT as f32;
                sample * glam::Vec2::lerp(glam::vec2(0.1, 0.1), glam::vec2(1.0, 1.0), scale * scale)
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let noise: [_; 16] = (0..16)
            .map(|_| {
                glam::vec2(
                    rand::random::<f32>() * 2.0 - 1.0,
                    rand::random::<f32>() * 2.0 - 1.0,
                )
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        Self { samples, noise }
    }
}

struct SsaoBlur {
    view: wgpu::TextureView,

    h_bind_group: wgpu::BindGroup,
    h_pipeline: wgpu::RenderPipeline,

    v_bind_group: wgpu::BindGroup,
    v_pipeline: wgpu::RenderPipeline,
}

impl SsaoBlur {
    fn new(renderer: &Renderer, output: &wgpu::TextureView) -> Self {
        let view = renderer
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("SsaoBlur temp texture"),
                size: wgpu::Extent3d {
                    width: renderer.surface_config.width,
                    height: renderer.surface_config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: SsaoPass::OUTPUT_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let shader = renderer
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("SsaoBlur shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ssao_blur.wgsl").into()),
            });

        let (h_bind_group, h_pipeline) = {
            let buffer = renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("SsaoBlur horizontal buffer"),
                    contents: bytemuck::cast_slice::<i32, _>(&[1, 0]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

            let bind_group_layout =
                renderer
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("SsaoBlur horizontal bind group layout"),
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: false,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: false,
                                    },
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Uniform,
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                        ],
                    });

            let bind_group = renderer
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("SsaoBlur horizontal bind group"),
                    layout: &bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(output),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: buffer.as_entire_binding(),
                        },
                    ],
                });

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("SsaoBlur horizontal pipeline layout"),
                        bind_group_layouts: &[&bind_group_layout],
                        push_constant_ranges: &[],
                    });

            let pipeline =
                renderer
                    .device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("SsaoBlur horizontal pipeline"),
                        layout: Some(&pipeline_layout),
                        multiview: None,
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: "main",
                            buffers: &[],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: "main",
                            targets: &[wgpu::ColorTargetState {
                                format: SsaoPass::OUTPUT_FORMAT,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }],
                        }),
                        primitive: wgpu::PrimitiveState::default(),
                        depth_stencil: None,
                        multisample: wgpu::MultisampleState::default(),
                    });

            (bind_group, pipeline)
        };

        let (v_bind_group, v_pipeline) = {
            let buffer = renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("SsaoBlur vertical buffer"),
                    contents: bytemuck::cast_slice::<i32, _>(&[0, 1]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

            let bind_group_layout =
                renderer
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("SsaoBlur vertical bind group layout"),
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: false,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: false,
                                    },
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Uniform,
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                        ],
                    });

            let bind_group = renderer
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("SsaoBlur vertical bind group"),
                    layout: &bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: buffer.as_entire_binding(),
                        },
                    ],
                });

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("SsaoBlur vertical pipeline layout"),
                        bind_group_layouts: &[&bind_group_layout],
                        push_constant_ranges: &[],
                    });

            let pipeline =
                renderer
                    .device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("SsaoBlur vertical pipeline"),
                        layout: Some(&pipeline_layout),
                        multiview: None,
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: "main",
                            buffers: &[],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: "main",
                            targets: &[wgpu::ColorTargetState {
                                format: SsaoPass::OUTPUT_FORMAT,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }],
                        }),
                        primitive: wgpu::PrimitiveState::default(),
                        depth_stencil: None,
                        multisample: wgpu::MultisampleState::default(),
                    });

            (bind_group, pipeline)
        };

        Self {
            view,

            h_bind_group,
            h_pipeline,

            v_bind_group,
            v_pipeline,
        }
    }

    fn render(&self, ctx: &mut RenderContext, output: &wgpu::TextureView) {
        {
            let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SsaoBlur horizontal"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &self.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            rpass.set_pipeline(&self.h_pipeline);
            rpass.set_bind_group(0, &self.h_bind_group, &[]);

            rpass.draw(0..3, 0..1);
        }

        {
            let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SsaoBlur vertical"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: output,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            rpass.set_pipeline(&self.v_pipeline);
            rpass.set_bind_group(0, &self.v_bind_group, &[]);

            rpass.draw(0..3, 0..1);
        }
    }
}
