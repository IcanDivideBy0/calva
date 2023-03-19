use crate::{AmbientLightPass, RenderContext, Renderer};

pub struct FxaaPass {
    pub output: wgpu::TextureView,

    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl FxaaPass {
    const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    pub fn new(renderer: &Renderer, lights: &wgpu::TextureView) -> Self {
        let output = Self::make_texture(renderer);

        let sampler = renderer.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Fxaa sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let bind_group_layout =
            renderer
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Fxaa bind group layout"),
                    entries: &[
                        // Sampler
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        // Input
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                    ],
                });

        let bind_group = Self::make_bind_group(renderer, lights, &bind_group_layout, &sampler);

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Fxaa pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let shader = renderer
            .device
            .create_shader_module(wgpu::include_wgsl!("fxaa.wgsl"));

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Fxaa pipeline"),
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
                    targets: &[Some(wgpu::ColorTargetState {
                        format: AmbientLightPass::OUTPUT_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
            });

        Self {
            output,

            sampler,
            bind_group_layout,
            bind_group,
            pipeline,
        }
    }

    pub fn rebind(&mut self, renderer: &Renderer, lights: &wgpu::TextureView) {
        self.output = Self::make_texture(renderer);
        self.bind_group =
            Self::make_bind_group(renderer, lights, &self.bind_group_layout, &self.sampler);
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Fxaa"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.output,
                resolve_target: None,
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

    fn make_texture(renderer: &Renderer) -> wgpu::TextureView {
        renderer
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Fxaa output"),
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
                view_formats: &[Self::OUTPUT_FORMAT],
            })
            .create_view(&Default::default())
    }

    fn make_bind_group(
        renderer: &Renderer,
        lights: &wgpu::TextureView,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Fxaa bind group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&lights),
                    },
                ],
            })
    }
}
