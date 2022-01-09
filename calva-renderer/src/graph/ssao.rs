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

    render_bundle: wgpu::RenderBundle,
    blur: blur::SsaoBlur,
    blit: blit::SsaoBlit,
}

impl Ssao {
    const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

    pub fn new(renderer: &Renderer, normal: &wgpu::TextureView, depth: &wgpu::TextureView) -> Self {
        let Renderer {
            device,
            surface_config,
            config,
            camera,
            ..
        } = renderer;

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
            contents: bytemuck::bytes_of(&SsaoUniform::new()),
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
            multiview: None,
            multisample: wgpu::MultisampleState::default(),
        });

        let render_bundle = {
            let mut encoder =
                device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                    label: Some("Ssao render bundle encoder"),
                    color_formats: &[Self::OUTPUT_FORMAT],
                    depth_stencil: None,
                    sample_count: 1,
                    multiview: None,
                });

            encoder.set_pipeline(&pipeline);
            encoder.set_bind_group(0, &renderer.config.bind_group, &[]);
            encoder.set_bind_group(1, &renderer.camera.bind_group, &[]);
            encoder.set_bind_group(2, &bind_group, &[]);

            encoder.draw(0..3, 0..1);

            encoder.finish(&wgpu::RenderBundleDescriptor {
                label: Some("Ssao render bundle"),
            })
        };

        let blur = blur::SsaoBlur::new(device, size, &view);
        let blit = blit::SsaoBlit::new(device, surface_config, &view);

        Self {
            view,

            render_bundle,
            blur,
            blit,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        ctx.encoder.push_debug_group("Ssao");

        ctx.encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
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
            })
            .execute_bundles(std::iter::once(&self.render_bundle));

        self.blur.render(ctx, &self.view);
        self.blit.render(ctx);

        ctx.encoder.pop_debug_group();
    }
}

mod blur {
    use crate::RenderContext;

    use super::Ssao;

    enum Direction {
        Horizontal,
        Vertical,
    }

    impl std::fmt::Display for Direction {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(match self {
                Direction::Horizontal => "horizontal",
                Direction::Vertical => "vertical",
            })
        }
    }

    pub struct SsaoBlur {
        temp: wgpu::TextureView,

        h_render_bundle: wgpu::RenderBundle,
        v_render_bundle: wgpu::RenderBundle,
    }

    impl SsaoBlur {
        pub fn new(
            device: &wgpu::Device,
            size: wgpu::Extent3d,
            output: &wgpu::TextureView,
        ) -> Self {
            let temp = device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("SsaoBlur temp texture"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: Ssao::OUTPUT_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                })
                .create_view(&wgpu::TextureViewDescriptor::default());

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("SsaoBlur bind group layout"),
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

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("SsaoBlur pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("SsaoBlur shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ssao.blur.wgsl").into()),
            });

            let make_render_bundle = |direction: Direction| {
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(format!("SsaoBlur {} bind group", direction).as_str()),
                    layout: &bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(match direction {
                            Direction::Horizontal => output,
                            Direction::Vertical => &temp,
                        }),
                    }],
                });

                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(format!("SsaoBlur {} pipeline", direction).as_str()),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: format!("fs_main_{}", direction).as_str(),
                        targets: &[wgpu::ColorTargetState {
                            format: Ssao::OUTPUT_FORMAT,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        }],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                });

                let mut encoder =
                    device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                        label: Some(
                            format!("SsaoBlur {} render bundle encoder", direction).as_str(),
                        ),
                        color_formats: &[Ssao::OUTPUT_FORMAT],
                        depth_stencil: None,
                        sample_count: 1,
                        multiview: None,
                    });

                encoder.set_pipeline(&pipeline);
                encoder.set_bind_group(0, &bind_group, &[]);

                encoder.draw(0..3, 0..1);

                encoder.finish(&wgpu::RenderBundleDescriptor {
                    label: Some(format!("SsaoBlur {} render bundle", direction).as_str()),
                })
            };

            let h_render_bundle = make_render_bundle(Direction::Horizontal);
            let v_render_bundle = make_render_bundle(Direction::Vertical);

            Self {
                temp,

                h_render_bundle,
                v_render_bundle,
            }
        }

        pub fn render(&self, ctx: &mut RenderContext, output: &wgpu::TextureView) {
            ctx.encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("SsaoBlur horizontal pass"),
                    color_attachments: &[wgpu::RenderPassColorAttachment {
                        view: &self.temp,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: true,
                        },
                    }],
                    depth_stencil_attachment: None,
                })
                .execute_bundles(std::iter::once(&self.h_render_bundle));

            ctx.encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("SsaoBlur vertical pass"),
                    color_attachments: &[wgpu::RenderPassColorAttachment {
                        view: output,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: true,
                        },
                    }],
                    depth_stencil_attachment: None,
                })
                .execute_bundles(std::iter::once(&self.v_render_bundle));
        }
    }
}

mod blit {
    use crate::{RenderContext, Renderer};

    pub struct SsaoBlit {
        render_bundle: wgpu::RenderBundle,
    }

    impl SsaoBlit {
        pub fn new(
            device: &wgpu::Device,
            surface_config: &wgpu::SurfaceConfiguration,
            ssao_result: &wgpu::TextureView,
        ) -> Self {
            let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("SsaoBlit shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ssao.blit.wgsl").into()),
            });

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("SsaoBlit bind group layout"),
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

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("SsaoBlit pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("SsaoBlit bind group"),
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(ssao_result),
                }],
            });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("SsaoBlit pipeline"),
                layout: Some(&pipeline_layout),
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
                multiview: None,
            });

            let render_bundle = {
                let mut encoder =
                    device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                        label: Some("SsaoBlit render bundle encoder"),
                        color_formats: &[surface_config.format],
                        depth_stencil: None,
                        sample_count: Renderer::MULTISAMPLE_STATE.count,
                        multiview: None,
                    });
                encoder.set_pipeline(&pipeline);
                encoder.set_bind_group(0, &bind_group, &[]);
                encoder.draw(0..3, 0..1);
                encoder.finish(&wgpu::RenderBundleDescriptor {
                    label: Some("SsaoBlit render bundle"),
                })
            };

            Self { render_bundle }
        }

        pub fn render(&self, ctx: &mut RenderContext) {
            ctx.encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("SsaoBlit pass"),
                    color_attachments: &[wgpu::RenderPassColorAttachment {
                        view: ctx.view,
                        resolve_target: ctx.resolve_target,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    }],
                    depth_stencil_attachment: None,
                })
                .execute_bundles(std::iter::once(&self.render_bundle));
        }
    }
}
