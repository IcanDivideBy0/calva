use wgpu::util::DeviceExt;

use crate::{RenderContext, Renderer};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SsaoConfig {
    pub radius: f32,
    pub bias: f32,
    pub power: f32,
}

impl SsaoConfig {
    pub const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as wgpu::BufferAddress;
}

impl Default for SsaoConfig {
    fn default() -> Self {
        Self {
            radius: 0.3,
            bias: 0.025,
            power: 1.0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SsaoRandomUniform {
    samples: [glam::Vec4; SsaoRandomUniform::SAMPLES_COUNT],
    noise: [glam::Vec4; 16],
}

impl SsaoRandomUniform {
    pub const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as wgpu::BufferAddress;

    const SAMPLES_COUNT: usize = 32;

    fn new() -> Self {
        let samples = (0..Self::SAMPLES_COUNT)
            .map(|i| {
                let sample = glam::vec4(
                    rand::random::<f32>() * 2.0 - 1.0,
                    rand::random::<f32>() * 2.0 - 1.0,
                    rand::random::<f32>(),
                    0.0,
                )
                .normalize();

                let scale = i as f32 / Self::SAMPLES_COUNT as f32;
                sample
                    * glam::Vec4::lerp(
                        glam::Vec4::splat(0.1),
                        glam::Vec4::splat(1.0),
                        scale * scale,
                    )
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let noise = (0..16)
            .map(|_| {
                glam::vec4(
                    rand::random::<f32>() * 2.0 - 1.0,
                    rand::random::<f32>() * 2.0 - 1.0,
                    0.0,
                    0.0,
                )
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        Self { samples, noise }
    }
}

pub struct SsaoPass<const WIDTH: u32, const HEIGHT: u32> {
    pub config: SsaoConfig,
    config_buffer: wgpu::Buffer,
    random_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,

    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,

    output: wgpu::TextureView,
    blur: blur::SsaoBlur<WIDTH, HEIGHT>,
    blit: blit::SsaoBlit,
}

impl<const WIDTH: u32, const HEIGHT: u32> SsaoPass<WIDTH, HEIGHT> {
    const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

    pub fn new(renderer: &Renderer, normal: &wgpu::TextureView, depth: &wgpu::TextureView) -> Self {
        let config = SsaoConfig::default();

        let config_buffer = renderer
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Ssao config buffer"),
                contents: bytemuck::bytes_of(&config),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let random_buffer = renderer
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Ssao uniforms buffer"),
                contents: bytemuck::bytes_of(&SsaoRandomUniform::new()),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let sampler = renderer.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Skybox sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout =
            renderer
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Ssao bind group layout"),
                    entries: &[
                        // config
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(SsaoConfig::SIZE),
                            },
                            count: None,
                        },
                        // random data
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(SsaoRandomUniform::SIZE),
                            },
                            count: None,
                        },
                        // sampler
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        // normals
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        // depth
                        wgpu::BindGroupLayoutEntry {
                            binding: 4,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: Renderer::MULTISAMPLE_STATE.count > 1,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Depth,
                            },
                            count: None,
                        },
                    ],
                });

        let bind_group = Self::create_bind_group(
            renderer,
            &bind_group_layout,
            &config_buffer,
            &random_buffer,
            &sampler,
            normal,
            depth,
        );

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Ssao pipeline layout"),
                    bind_group_layouts: &[&renderer.camera.bind_group_layout, &bind_group_layout],
                    push_constant_ranges: &[],
                });

        let shader = renderer
            .device
            .create_shader_module(wgpu::include_wgsl!("shaders/ssao.wgsl"));

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                    targets: &[Some(wgpu::ColorTargetState {
                        format: Self::OUTPUT_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multiview: None,
                multisample: Default::default(),
            });

        let output =
            Self::make_texture(renderer, Some("Ssao output")).create_view(&Default::default());

        let blur = blur::SsaoBlur::new(renderer, &output);
        let blit = blit::SsaoBlit::new(renderer, &output);

        Self {
            config,
            config_buffer,
            random_buffer,
            sampler,

            bind_group_layout,
            bind_group,
            pipeline,

            output,
            blur,
            blit,
        }
    }

    pub fn resize(
        &mut self,
        renderer: &Renderer,
        normal: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) {
        self.bind_group = Self::create_bind_group(
            renderer,
            &self.bind_group_layout,
            &self.config_buffer,
            &self.random_buffer,
            &self.sampler,
            normal,
            depth,
        );
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        if self.config.radius == 0.0 || self.config.power == 0.0 {
            return;
        }

        ctx.encoder.profile_start("Ssao");

        ctx.queue
            .write_buffer(&self.config_buffer, 0, bytemuck::bytes_of(&self.config));

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Ssao[render]"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.output,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &ctx.camera.bind_group, &[]);
        rpass.set_bind_group(1, &self.bind_group, &[]);

        rpass.draw(0..3, 0..1);

        drop(rpass);

        self.blur.render(ctx, &self.output);
        self.blit.render(ctx);

        ctx.encoder.profile_end();
    }

    fn make_texture(renderer: &Renderer, label: wgpu::Label<'static>) -> wgpu::Texture {
        renderer.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::OUTPUT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[Self::OUTPUT_FORMAT],
        })
    }

    fn create_bind_group(
        renderer: &Renderer,
        layout: &wgpu::BindGroupLayout,
        config_buffer: &wgpu::Buffer,
        random_buffer: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        normal: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Ssao bind group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: config_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: random_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(normal),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(depth),
                    },
                ],
            })
    }
}

mod blur {
    use crate::{RenderContext, Renderer};

    use super::SsaoPass;

    #[derive(Clone, Copy)]
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

    pub struct SsaoBlur<const WIDTH: u32, const HEIGHT: u32> {
        temp: wgpu::TextureView,

        h_pass: wgpu::RenderBundle,
        v_pass: wgpu::RenderBundle,
    }

    impl<const WIDTH: u32, const HEIGHT: u32> SsaoBlur<WIDTH, HEIGHT> {
        pub fn new(renderer: &Renderer, output: &wgpu::TextureView) -> Self {
            let temp =
                SsaoPass::<WIDTH, HEIGHT>::make_texture(renderer, Some("SsaoBlur temp texture"))
                    .create_view(&Default::default());

            let bind_group_layout =
                renderer
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("SsaoBlur pipeline layout"),
                        bind_group_layouts: &[&bind_group_layout],
                        push_constant_ranges: &[],
                    });

            let shader = renderer
                .device
                .create_shader_module(wgpu::include_wgsl!("shaders/ssao.blur.wgsl"));

            let make_render_bundle = |direction: Direction| {
                let bind_group = renderer
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some(format!("SsaoBlur[{}] bind group", direction).as_str()),
                        layout: &bind_group_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(match direction {
                                Direction::Horizontal => output,
                                Direction::Vertical => &temp,
                            }),
                        }],
                    });

                let pipeline =
                    renderer
                        .device
                        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                            label: Some(format!("SsaoBlur[{}] pipeline", direction).as_str()),
                            layout: Some(&pipeline_layout),
                            vertex: wgpu::VertexState {
                                module: &shader,
                                entry_point: "vs_main",
                                buffers: &[],
                            },
                            fragment: Some(wgpu::FragmentState {
                                module: &shader,
                                entry_point: format!("fs_main_{}", direction).as_str(),
                                targets: &[Some(wgpu::ColorTargetState {
                                    format: SsaoPass::<WIDTH, HEIGHT>::OUTPUT_FORMAT,
                                    blend: None,
                                    write_mask: wgpu::ColorWrites::ALL,
                                })],
                            }),
                            primitive: Default::default(),
                            depth_stencil: None,
                            multisample: Default::default(),
                            multiview: None,
                        });

                let mut encoder = renderer.device.create_render_bundle_encoder(
                    &wgpu::RenderBundleEncoderDescriptor {
                        label: Some(format!("SsaoBlur[{}] render bundle", direction).as_str()),
                        color_formats: &[Some(SsaoPass::<WIDTH, HEIGHT>::OUTPUT_FORMAT)],
                        depth_stencil: None,
                        sample_count: 1,
                        multiview: None,
                    },
                );

                encoder.set_pipeline(&pipeline);
                encoder.set_bind_group(0, &bind_group, &[]);

                encoder.draw(0..3, 0..1);

                encoder.finish(&Default::default())
            };

            let h_pass = make_render_bundle(Direction::Horizontal);
            let v_pass = make_render_bundle(Direction::Vertical);

            Self {
                temp,

                h_pass,
                v_pass,
            }
        }

        pub fn render(&self, ctx: &mut RenderContext, output: &wgpu::TextureView) {
            ctx.encoder.profile_start("Ssao[blur]");

            ctx.encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Ssao[blur][horizontal]"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.temp,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: None,
                })
                .execute_bundles(std::iter::once(&self.h_pass));

            ctx.encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Ssao[blur][vertical]"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: output,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: None,
                })
                .execute_bundles(std::iter::once(&self.v_pass));

            ctx.encoder.profile_end();
        }
    }
}

mod blit {
    use crate::{RenderContext, Renderer};

    pub struct SsaoBlit {
        bind_group: wgpu::BindGroup,
        pipeline: wgpu::RenderPipeline,
    }

    impl SsaoBlit {
        pub fn new(renderer: &Renderer, ssao_output: &wgpu::TextureView) -> Self {
            let bind_group_layout =
                renderer
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("SsaoBlit bind group layout"),
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: false,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                                count: None,
                            },
                        ],
                    });

            let sampler = renderer.device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });

            let bind_group = renderer
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("SsaoBlit bind group"),
                    layout: &bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(ssao_output),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                });

            let shader = renderer
                .device
                .create_shader_module(wgpu::include_wgsl!("shaders/ssao.blit.wgsl"));

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("SsaoBlit pipeline layout"),
                        bind_group_layouts: &[&bind_group_layout],
                        push_constant_ranges: &[],
                    });

            let pipeline =
                renderer
                    .device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                            targets: &[Some(wgpu::ColorTargetState {
                                format: renderer.surface_config.format,
                                blend: Some(wgpu::BlendState {
                                    color: wgpu::BlendComponent::OVER,
                                    alpha: wgpu::BlendComponent::OVER,
                                }),
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        primitive: Default::default(),
                        depth_stencil: None,
                        multisample: Renderer::MULTISAMPLE_STATE,
                        multiview: None,
                    });

            Self {
                bind_group,
                pipeline,
            }
        }

        pub fn render(&self, ctx: &mut RenderContext) {
            let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Ssao[blit]"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: ctx.output.view,
                    resolve_target: ctx.output.resolve_target,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &self.bind_group, &[]);

            rpass.draw(0..3, 0..1);
        }
    }
}
