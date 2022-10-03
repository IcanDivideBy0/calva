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

    const SAMPLES_COUNT: usize = 64;

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

pub struct SsaoPass {
    pub config: SsaoConfig,
    config_buffer: wgpu::Buffer,
    random_buffer: wgpu::Buffer,

    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,

    output: wgpu::TextureView,
    blur: blur::SsaoBlur,
    blit: blit::SsaoBlit,
}

impl SsaoPass {
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
                        // depth
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
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
                            binding: 3,
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

        let bind_group = Self::create_bind_group(
            renderer,
            &bind_group_layout,
            &config_buffer,
            &random_buffer,
            depth,
            normal,
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
        self.output =
            Self::make_texture(renderer, Some("Ssao output")).create_view(&Default::default());

        self.bind_group = Self::create_bind_group(
            renderer,
            &self.bind_group_layout,
            &self.config_buffer,
            &self.random_buffer,
            depth,
            normal,
        );

        self.blur.resize(renderer, &self.output);
        self.blit.resize(renderer, &self.output);
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
                width: renderer.surface_config.width,
                height: renderer.surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::OUTPUT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        })
    }

    fn create_bind_group(
        renderer: &Renderer,
        layout: &wgpu::BindGroupLayout,
        config_buffer: &wgpu::Buffer,
        random_buffer: &wgpu::Buffer,
        depth: &wgpu::TextureView,
        normal: &wgpu::TextureView,
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
                        resource: wgpu::BindingResource::TextureView(depth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(normal),
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

    pub struct SsaoBlur {
        temp: wgpu::TextureView,

        bind_group_layout: wgpu::BindGroupLayout,

        h_pass: (wgpu::BindGroup, wgpu::RenderPipeline),
        v_pass: (wgpu::BindGroup, wgpu::RenderPipeline),
    }

    impl SsaoBlur {
        pub fn new(renderer: &Renderer, output: &wgpu::TextureView) -> Self {
            let temp = SsaoPass::make_texture(renderer, Some("SsaoBlur temp texture"))
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
                let bind_group =
                    Self::create_bind_group(renderer, &bind_group_layout, &temp, output, direction);

                let pipeline =
                    renderer
                        .device
                        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                                targets: &[Some(wgpu::ColorTargetState {
                                    format: SsaoPass::OUTPUT_FORMAT,
                                    blend: None,
                                    write_mask: wgpu::ColorWrites::ALL,
                                })],
                            }),
                            primitive: Default::default(),
                            depth_stencil: None,
                            multisample: Default::default(),
                            multiview: None,
                        });

                (bind_group, pipeline)
            };

            let h_pass = make_render_bundle(Direction::Horizontal);
            let v_pass = make_render_bundle(Direction::Vertical);

            Self {
                temp,

                bind_group_layout,

                h_pass,
                v_pass,
            }
        }

        pub fn resize(&mut self, renderer: &Renderer, output: &wgpu::TextureView) {
            self.temp = SsaoPass::make_texture(renderer, Some("SsaoBlur temp texture"))
                .create_view(&Default::default());

            self.h_pass.0 = Self::create_bind_group(
                renderer,
                &self.bind_group_layout,
                &self.temp,
                output,
                Direction::Horizontal,
            );

            self.v_pass.0 = Self::create_bind_group(
                renderer,
                &self.bind_group_layout,
                &self.temp,
                output,
                Direction::Vertical,
            );
        }

        pub fn render(&self, ctx: &mut RenderContext, output: &wgpu::TextureView) {
            ctx.encoder.profile_start("Ssao[blur]");

            let mut hpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
            });

            hpass.set_pipeline(&self.h_pass.1);
            hpass.set_bind_group(0, &self.h_pass.0, &[]);

            hpass.draw(0..3, 0..1);

            drop(hpass);

            let mut vpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
            });

            vpass.set_pipeline(&self.v_pass.1);
            vpass.set_bind_group(0, &self.v_pass.0, &[]);

            vpass.draw(0..3, 0..1);

            drop(vpass);

            ctx.encoder.profile_end();
        }

        fn create_bind_group(
            renderer: &Renderer,
            bind_group_layout: &wgpu::BindGroupLayout,

            temp: &wgpu::TextureView,
            output: &wgpu::TextureView,

            direction: Direction,
        ) -> wgpu::BindGroup {
            renderer
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(format!("SsaoBlur {} bind group", direction).as_str()),
                    layout: bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(match direction {
                            Direction::Horizontal => output,
                            Direction::Vertical => temp,
                        }),
                    }],
                })
        }
    }
}

mod blit {
    use crate::{RenderContext, Renderer};

    pub struct SsaoBlit {
        bind_group_layout: wgpu::BindGroupLayout,
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

            let bind_group = Self::create_bind_group(renderer, &bind_group_layout, ssao_output);

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
                bind_group_layout,
                bind_group,
                pipeline,
            }
        }

        pub fn resize(&mut self, renderer: &Renderer, ssao_output: &wgpu::TextureView) {
            self.bind_group =
                Self::create_bind_group(renderer, &self.bind_group_layout, ssao_output);
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

        fn create_bind_group(
            renderer: &Renderer,
            bind_group_layout: &wgpu::BindGroupLayout,
            view: &wgpu::TextureView,
        ) -> wgpu::BindGroup {
            renderer
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("SsaoBlit bind group"),
                    layout: bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    }],
                })
        }
    }
}
