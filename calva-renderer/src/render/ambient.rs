use wgpu::util::DeviceExt;

use crate::{RenderContext, Renderer};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AmbientConfig {
    pub factor: f32,
}

impl AmbientConfig {
    pub const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as wgpu::BufferAddress;
}

impl Default for AmbientConfig {
    fn default() -> Self {
        Self { factor: 0.1 }
    }
}

pub struct AmbientPass {
    pub config: AmbientConfig,
    config_buffer: wgpu::Buffer,

    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl AmbientPass {
    pub fn new(renderer: &Renderer, albedo: &wgpu::TextureView) -> Self {
        let config = AmbientConfig::default();

        let config_buffer = renderer
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Ambient config buffer"),
                contents: bytemuck::bytes_of(&config),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let bind_group_layout =
            renderer
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Ambient bind group layout"),
                    entries: &[
                        // ambient factor
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(AmbientConfig::SIZE),
                            },
                            count: None,
                        },
                        // albedo
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

        let bind_group =
            Self::make_bind_group(&renderer.device, &bind_group_layout, &config_buffer, albedo);

        let shader = renderer
            .device
            .create_shader_module(wgpu::include_wgsl!("shaders/ambient.wgsl"));

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Ambient pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[wgpu::PushConstantRange {
                        stages: wgpu::ShaderStages::FRAGMENT,
                        range: 0..(std::mem::size_of::<f32>() as _),
                    }],
                });

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Ambient pipeline"),
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
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Renderer::MULTISAMPLE_STATE,
                multiview: None,
            });

        Self {
            config,
            config_buffer,

            bind_group_layout,
            bind_group,
            pipeline,
        }
    }

    pub fn resize(&mut self, renderer: &Renderer, albedo: &wgpu::TextureView) {
        self.bind_group = Self::make_bind_group(
            &renderer.device,
            &self.bind_group_layout,
            &self.config_buffer,
            albedo,
        );
    }

    pub fn render(&self, ctx: &mut RenderContext, gamma: f32) {
        ctx.queue
            .write_buffer(&self.config_buffer, 0, bytemuck::bytes_of(&self.config));

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Ambient"),
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
        rpass.set_push_constants(wgpu::ShaderStages::FRAGMENT, 0, bytemuck::bytes_of(&gamma));

        rpass.draw(0..3, 0..1);

        drop(rpass);
    }

    fn make_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        config_buffer: &wgpu::Buffer,
        albedo: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Ambient bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: config_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(albedo),
                },
            ],
        })
    }
}
