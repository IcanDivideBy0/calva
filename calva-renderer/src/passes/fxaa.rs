use crate::{AmbientLightPassOutputs, RenderContext, Resource, ResourcesManager};

pub struct FxaaPassOutputs {
    pub output: wgpu::Texture,
    pub output_view: wgpu::TextureView,
}

impl Resource for FxaaPassOutputs {
    fn instanciate(resources: &ResourcesManager) -> Self {
        let device = resources.read::<wgpu::Device>();
        let ambient_light_outputs = resources.read::<AmbientLightPassOutputs>();

        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Fxaa output"),
            size: ambient_light_outputs.output.size(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: ambient_light_outputs.output.format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[ambient_light_outputs.output.format()],
        });

        let output_view = output.create_view(&Default::default());

        Self {
            output,
            output_view,
        }
    }

    fn update(&mut self, resources: &ResourcesManager) -> anyhow::Result<()> {
        let ambient_light_outputs = resources.read::<AmbientLightPassOutputs>();

        if ambient_light_outputs.output.size() != self.output.size() {
            *self = Self::instanciate(resources);
        }

        Ok(())
    }

    fn update_dependencies() -> impl IntoIterator<Item = std::any::TypeId> {
        [std::any::TypeId::of::<AmbientLightPassOutputs>()]
    }
}

pub struct FxaaPass {
    resources: ResourcesManager,

    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl FxaaPass {
    pub fn new(resources: &ResourcesManager) -> Self {
        let resources = resources.clone();
        let device = resources.read::<wgpu::Device>();
        let ambient_light_outputs = resources.read::<AmbientLightPassOutputs>();

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

        let bind_group = Self::make_bind_group(
            &device,
            &bind_group_layout,
            &sampler,
            &ambient_light_outputs.output,
        );

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Fxaa pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Fxaa shader"),
            source: wgpu::ShaderSource::Wgsl(wesl::include_wesl!("passes::fxaa").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Fxaa pipeline"),
            layout: Some(&pipeline_layout),
            multiview_mask: None,
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
                    format: ambient_light_outputs.output.format(),
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
            resources,

            sampler,
            bind_group_layout,
            bind_group,
            pipeline,
        }
    }

    pub fn rebind(&mut self) {
        let device = self.resources.read::<wgpu::Device>();
        let ambient_light_outputs = self.resources.read::<AmbientLightPassOutputs>();

        self.bind_group = Self::make_bind_group(
            &device,
            &self.bind_group_layout,
            &self.sampler,
            &ambient_light_outputs.output,
        );
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        let fxaa_outputs = self.resources.read::<FxaaPassOutputs>();

        let mut rpass = ctx.encoder.scoped_render_pass(
            "Fxaa",
            wgpu::RenderPassDescriptor {
                label: Some("Fxaa"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &fxaa_outputs.output_view,
                    depth_slice: None,
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
        rpass.set_bind_group(0, &self.bind_group, &[]);

        rpass.draw(0..3, 0..1);
    }

    fn make_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        output: &wgpu::Texture,
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
                        &output.create_view(&Default::default()),
                    ),
                },
            ],
        })
    }
}
