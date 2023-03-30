use crate::{RenderContext, UniformBuffer};

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ToneMappingConfig {
    pub exposure: f32,
    pub gamma: f32,
}

#[cfg(feature = "egui")]
impl egui::Widget for &mut ToneMappingConfig {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        egui::CollapsingHeader::new("Tone mapping")
            .default_open(true)
            .show(ui, |ui| {
                ui.add(egui::Slider::new(&mut self.exposure, -10.0..=10.0).text("Exposure"));
                ui.add(egui::Slider::new(&mut self.gamma, 0.0..=5.0).text("Gamma"));
            })
            .header_response
    }
}

impl Default for ToneMappingConfig {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            gamma: 1.0,
        }
    }
}

pub struct ToneMappingPassInputs<'a> {
    pub format: wgpu::TextureFormat,
    pub input: &'a wgpu::Texture,
}

pub struct ToneMappingPass {
    pub config: UniformBuffer<ToneMappingConfig>,

    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl ToneMappingPass {
    pub fn new(device: &wgpu::Device, inputs: ToneMappingPassInputs) -> Self {
        let config = UniformBuffer::new(device, ToneMappingConfig::default());

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ToneMapping bind group layout"),
            entries: &[
                // hdr
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
            ],
        });

        let bind_group = Self::make_bind_group(device, &bind_group_layout, &inputs);

        let shader = device.create_shader_module(wgpu::include_wgsl!("tone_mapping.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ToneMapping pipeline layout"),
            bind_group_layouts: &[&config.bind_group_layout, &bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ToneMapping pipeline"),
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
                    format: inputs.format,
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
            config,

            bind_group_layout,
            bind_group,
            pipeline,
        }
    }

    pub fn rebind(&mut self, device: &wgpu::Device, input: ToneMappingPassInputs) {
        self.bind_group = Self::make_bind_group(device, &self.bind_group_layout, &input);
    }

    pub fn update(&mut self, queue: &wgpu::Queue) {
        self.config.update(queue);
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ToneMapping"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.frame,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.config.bind_group, &[]);
        rpass.set_bind_group(1, &self.bind_group, &[]);

        rpass.draw(0..3, 0..1);
    }

    fn make_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        inputs: &ToneMappingPassInputs,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ToneMapping bind group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &inputs.input.create_view(&Default::default()),
                ),
            }],
        })
    }
}
