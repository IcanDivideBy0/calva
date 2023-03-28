use crate::RenderContext;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AmbientLightConfig {
    pub factor: f32,
}

impl Default for AmbientLightConfig {
    fn default() -> Self {
        Self { factor: 0.1 }
    }
}

pub struct AmbientLightPassInputs<'a> {
    pub albedo: &'a wgpu::Texture,
    pub emissive: &'a wgpu::Texture,
}

pub struct AmbientLightPassOutputs {
    pub output: wgpu::Texture,
}

pub struct AmbientLightPass {
    pub outputs: AmbientLightPassOutputs,
    output_view: wgpu::TextureView,

    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl AmbientLightPass {
    pub fn new(device: &wgpu::Device, inputs: AmbientLightPassInputs) -> Self {
        let outputs = Self::make_outputs(device, &inputs);
        let output_view = outputs.output.create_view(&Default::default());

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("AmbientLight bind group layout"),
            entries: &[
                // albedo
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                // emissive
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
            ],
        });

        let bind_group = Self::make_bind_group(device, &bind_group_layout, &inputs);

        let shader = device.create_shader_module(wgpu::include_wgsl!("ambient_light.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("AmbientLight pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::FRAGMENT,
                range: 0..(std::mem::size_of::<f32>() as _),
            }],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("AmbientLight pipeline"),
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
                    format: outputs.output.format(),
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
        });

        Self {
            outputs,
            output_view,

            bind_group_layout,
            bind_group,
            pipeline,
        }
    }

    pub fn rebind(&mut self, device: &wgpu::Device, inputs: AmbientLightPassInputs) {
        self.outputs = Self::make_outputs(device, &inputs);
        self.output_view = self.outputs.output.create_view(&Default::default());

        self.bind_group = Self::make_bind_group(device, &self.bind_group_layout, &inputs);
    }

    pub fn render(&self, ctx: &mut RenderContext, config: &AmbientLightConfig) {
        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("AmbientLight"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.output_view,
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
        rpass.set_push_constants(wgpu::ShaderStages::FRAGMENT, 0, bytemuck::bytes_of(config));

        rpass.draw(0..3, 0..1);
    }

    fn make_outputs(
        device: &wgpu::Device,
        inputs: &AmbientLightPassInputs,
    ) -> AmbientLightPassOutputs {
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("AmbientLight output"),
            size: wgpu::Extent3d {
                depth_or_array_layers: 1,
                ..inputs.albedo.size()
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[wgpu::TextureFormat::Rgba16Float],
        });

        AmbientLightPassOutputs { output }
    }

    fn make_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        inputs: &AmbientLightPassInputs,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("AmbientLight bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.albedo.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.emissive.create_view(&Default::default()),
                    ),
                },
            ],
        })
    }
}
