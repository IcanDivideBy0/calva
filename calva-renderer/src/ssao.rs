use wgpu::util::DeviceExt;

use crate::{RenderContext, Renderer};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SsaoUniform {
    samples: [glam::Vec2; SsaoUniform::SAMPLES_COUNT],
    noise: [glam::Vec2; 16],
}

impl SsaoUniform {
    const SAMPLES_COUNT: usize = 16;

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

pub struct Ssao {
    view: wgpu::TextureView,

    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    blur: SsaoBlur,
    blit: SsaoBlit,
}

impl Ssao {
    const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

    pub fn new(
        Renderer {
            device,
            surface_config,
            config,
            camera,
            ..
        }: &Renderer,

        normal: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };

        let view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Ssao view"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::OUTPUT_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let random_data_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Ssao random data buffer"),
            contents: bytemuck::cast_slice(&[SsaoUniform::new()]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Ssao bind group layout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Ssao bind group"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Ssao pipeline layout"),
            bind_group_layouts: &[
                &config.bind_group_layout,
                &camera.bind_group_layout,
                &bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Ssao shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ssao.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Ssao pipeline"),
            layout: Some(&pipeline_layout),
            multiview: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
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

        let blur = SsaoBlur::new(device, size, &view);
        let blit = SsaoBlit::new(device, surface_config, &view);

        Self {
            view,

            bind_group,
            pipeline,
            blur,
            blit,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        ctx.encoder.push_debug_group("Ssao");

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Ssao render pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &self.view,
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
        drop(rpass);

        self.blur.render(ctx, &self.view);
        self.blit.render(ctx);

        ctx.encoder.pop_debug_group();
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
    fn new(device: &wgpu::Device, size: wgpu::Extent3d, output: &wgpu::TextureView) -> Self {
        let view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Ssao blur temp texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Ssao::OUTPUT_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Ssao blur shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ssao.blur.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Ssao blur bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                },
                count: None,
            }],
        });

        let (h_bind_group, h_pipeline) = {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Ssao blur horizontal bind group"),
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(output),
                }],
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Ssao blur horizontal pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Ssao blur horizontal pipeline"),
                layout: Some(&pipeline_layout),
                multiview: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main_horizontal",
                    targets: &[wgpu::ColorTargetState {
                        format: Ssao::OUTPUT_FORMAT,
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
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Ssao blur vertical bind group"),
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                }],
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Ssao blur vertical pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Ssao blur vertical pipeline"),
                layout: Some(&pipeline_layout),
                multiview: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main_vertical",
                    targets: &[wgpu::ColorTargetState {
                        format: Ssao::OUTPUT_FORMAT,
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
        // horizontal pass
        {
            let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Ssao blur horizontal pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &self.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            rpass.set_pipeline(&self.h_pipeline);
            rpass.set_bind_group(0, &self.h_bind_group, &[]);

            rpass.draw(0..3, 0..1);
        }

        // vertical pass
        {
            let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Ssao blur vertical pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: output,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
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

struct SsaoBlit {
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl SsaoBlit {
    fn new(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        ssao_result: &wgpu::TextureView,
    ) -> Self {
        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Ssao blit shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ssao.blit.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Ssao blit bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Ssao blit bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(ssao_result),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Ssao blit pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Ssao blit pipeline"),
            layout: Some(&pipeline_layout),
            multiview: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::OVER,
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Renderer::MULTISAMPLE_STATE,
        });

        Self {
            bind_group,
            pipeline,
        }
    }

    fn render(&self, ctx: &mut RenderContext) {
        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Ssao blit pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: ctx.view,
                resolve_target: ctx.resolve_target,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);

        rpass.draw(0..3, 0..1);
    }
}
