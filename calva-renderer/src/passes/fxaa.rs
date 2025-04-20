use crate::RenderContext;

pub struct FxaaPassInputs<'a> {
    pub input: &'a wgpu::Texture,
}

pub struct FxaaPassOutputs {
    pub output: wgpu::Texture,
}

pub struct FxaaPass {
    pub outputs: FxaaPassOutputs,
    output_view: wgpu::TextureView,

    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl FxaaPass {
    pub fn new(device: &wgpu::Device, inputs: FxaaPassInputs) -> Self {
        let outputs = FxaaPassOutputs {
            output: Self::make_texture(device, &inputs),
        };
        let output_view = outputs.output.create_view(&Default::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Fxaa sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let bind_group = Self::make_bind_group(device, &bind_group_layout, &sampler, &inputs);

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Fxaa pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("fxaa.wgsl"));

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Fxaa pipeline"),
            layout: Some(&pipeline_layout),
            multiview: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: inputs.input.format(),
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            cache: None,
        });

        Self {
            outputs,
            output_view,

            sampler,
            bind_group_layout,
            bind_group,
            pipeline,
        }
    }

    pub fn rebind(&mut self, device: &wgpu::Device, inputs: FxaaPassInputs) {
        self.outputs = FxaaPassOutputs {
            output: Self::make_texture(device, &inputs),
        };
        self.output_view = self.outputs.output.create_view(&Default::default());

        self.bind_group =
            Self::make_bind_group(device, &self.bind_group_layout, &self.sampler, &inputs);
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        let color_attachments = [Some(wgpu::RenderPassColorAttachment {
            view: &self.output_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })];

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Fxaa"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: None,
            ..Default::default()
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);

        rpass.draw(0..3, 0..1);
    }

    fn make_texture(device: &wgpu::Device, inputs: &FxaaPassInputs) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Fxaa output"),
            size: wgpu::Extent3d {
                depth_or_array_layers: 1,
                ..inputs.input.size()
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: inputs.input.format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[inputs.input.format()],
        })
    }

    fn make_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        inputs: &FxaaPassInputs,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Fxaa bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.input.create_view(&Default::default()),
                    ),
                },
            ],
        })
    }
}
