use crate::{CameraManager, RenderContext, ResourceRef, ResourcesManager, UniformBuffer};

mod blit;
mod blur;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SsaoConfig {
    pub radius: f32,
    pub bias: f32,
    pub power: f32,
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

#[cfg(feature = "egui")]
impl egui::Widget for &mut SsaoConfig {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        egui::CollapsingHeader::new("SSAO")
            .default_open(true)
            .show(ui, |ui| {
                ui.add(egui::Slider::new(&mut self.radius, 0.0..=4.0).text("Radius"));
                ui.add(egui::Slider::new(&mut self.bias, 0.0..=0.1).text("Bias"));
                ui.add(egui::Slider::new(&mut self.power, 0.0..=8.0).text("Power"));
            })
            .header_response
    }
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct SsaoRandom {
    samples: [glam::Vec4; SsaoRandom::SAMPLES_COUNT],
    noise: [glam::Vec4; 16],
}

impl SsaoRandom {
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

pub struct SsaoPassInputs<'a> {
    pub normal: &'a wgpu::Texture,
    pub depth: &'a wgpu::Texture,
    pub output: &'a wgpu::Texture,
}

pub struct SsaoPass<const WIDTH: u32, const HEIGHT: u32> {
    pub config: UniformBuffer<SsaoConfig>,
    random: UniformBuffer<SsaoRandom>,

    camera: ResourceRef<CameraManager>,

    output_view: wgpu::TextureView,

    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,

    blur: blur::SsaoBlurPass<WIDTH, HEIGHT>,
    blit: blit::SsaoBlitPass,
}

impl<const WIDTH: u32, const HEIGHT: u32> SsaoPass<WIDTH, HEIGHT> {
    pub fn new(
        device: &wgpu::Device,
        resources: &ResourcesManager,
        inputs: SsaoPassInputs,
    ) -> Self {
        let config = UniformBuffer::new(device, SsaoConfig::default());
        let random = UniformBuffer::new(device, SsaoRandom::new());

        let camera = resources.get::<CameraManager>();

        let output = Self::make_texture(device, Some("Ssao output"));
        let output_view = output.create_view(&Default::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Ssao sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Ssao bind group layout"),
            entries: &[
                // sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // normals
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
                // depth
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Depth,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = Self::make_bind_group(device, &bind_group_layout, &sampler, &inputs);

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Ssao pipeline layout"),
            bind_group_layouts: &[
                &camera.get().bind_group_layout,
                &config.bind_group_layout,
                &random.bind_group_layout,
                &bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Ssao shader"),
            source: wgpu::ShaderSource::Wgsl(wesl::include_wesl!("ssao").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Ssao pipeline"),
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
                    format: output.format(),
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multiview: None,
            multisample: Default::default(),
            cache: None,
        });

        let blur = blur::SsaoBlurPass::new(device, &output);
        let blit = blit::SsaoBlitPass::new(device, &output, inputs.output);

        Self {
            config,
            random,

            camera,

            sampler,

            bind_group_layout,
            bind_group,
            pipeline,

            output_view,
            blur,
            blit,
        }
    }

    pub fn rebind(&mut self, device: &wgpu::Device, inputs: SsaoPassInputs) {
        self.bind_group =
            Self::make_bind_group(device, &self.bind_group_layout, &self.sampler, &inputs);

        self.blit.rebind(inputs.output);
    }

    pub fn update(&mut self, queue: &wgpu::Queue) {
        self.config.update(queue);
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        let mut encoder = ctx.encoder.scope("Ssao");

        let camera = self.camera.get();

        let mut rpass = encoder.scoped_render_pass(
            "Ssao[render]",
            wgpu::RenderPassDescriptor {
                label: Some("Ssao[render]"),
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
        rpass.set_bind_group(0, &camera.bind_group, &[]);
        rpass.set_bind_group(1, &self.config.bind_group, &[]);
        rpass.set_bind_group(2, &self.random.bind_group, &[]);
        rpass.set_bind_group(3, &self.bind_group, &[]);

        rpass.draw(0..3, 0..1);

        drop(rpass);

        self.blur.render(&mut encoder);
        self.blit.render(&mut encoder);
    }

    fn make_texture(device: &wgpu::Device, label: wgpu::Label<'static>) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[wgpu::TextureFormat::R8Unorm],
        })
    }

    fn make_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        inputs: &SsaoPassInputs,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Ssao bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.normal.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&inputs.depth.create_view(
                        &wgpu::TextureViewDescriptor {
                            aspect: wgpu::TextureAspect::DepthOnly,
                            ..Default::default()
                        },
                    )),
                },
            ],
        })
    }
}
