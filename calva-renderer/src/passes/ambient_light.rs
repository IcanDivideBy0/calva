use crate::{RenderContext, UniformBuffer};

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AmbientLightConfig {
    pub color: [f32; 3],
    pub strength: f32,
}

impl Default for AmbientLightConfig {
    fn default() -> Self {
        // Blender defaults
        Self {
            color: [0.05; 3],
            strength: 1.0,
        }
    }
}

#[cfg(feature = "egui")]
impl egui::Widget for &mut AmbientLightConfig {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        egui::CollapsingHeader::new("Ambient light")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    egui::color_picker::color_edit_button_rgb(ui, &mut self.color);
                    ui.add(
                        egui::Label::new(egui::WidgetText::from("Color"))
                            .wrap_mode(egui::TextWrapMode::Truncate),
                    );
                });

                ui.add(egui::Slider::new(&mut self.strength, 0.0..=1.0).text("Strength"));
            })
            .header_response
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
    pub config: UniformBuffer<AmbientLightConfig>,
    pub outputs: AmbientLightPassOutputs,
    output_view: wgpu::TextureView,

    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl AmbientLightPass {
    pub fn new(device: &wgpu::Device, inputs: AmbientLightPassInputs) -> Self {
        let config = UniformBuffer::new(device, AmbientLightConfig::default());

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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("AmbientLight shader"),
            source: wgpu::ShaderSource::Wgsl(wesl::include_wesl!("ambient_light").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("AmbientLight pipeline layout"),
            bind_group_layouts: &[&config.bind_group_layout, &bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("AmbientLight pipeline"),
            layout: Some(&pipeline_layout),
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
                    format: outputs.output.format(),
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        Self {
            config,
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

    pub fn update(&mut self, queue: &wgpu::Queue) {
        self.config.update(queue);
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        let mut rpass = ctx.encoder.scoped_render_pass(
            "AmbientLight",
            wgpu::RenderPassDescriptor {
                label: Some("AmbientLight"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            },
        );

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.config.bind_group, &[]);
        rpass.set_bind_group(1, &self.bind_group, &[]);

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
