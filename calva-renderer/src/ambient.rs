use crate::RenderContext;
use crate::Renderer;

pub struct Ambient {
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl Ambient {
    pub fn new(renderer: &Renderer, albedo: &wgpu::TextureView) -> Self {
        let Renderer {
            device,
            config,
            surface_config,
            ..
        } = renderer;

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Ambient bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: Renderer::MULTISAMPLE_STATE.count > 1,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Ambient bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(albedo),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Ambient pipeline layout"),
            bind_group_layouts: &[&config.bind_group_layout, &bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Ambient shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ambient.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Ambient pipeline"),
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
                targets: &[wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Renderer::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Greater,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: Renderer::MULTISAMPLE_STATE,
        });

        Self {
            bind_group,
            pipeline,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        ctx.encoder.push_debug_group("Ambient");

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Ambient render pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: ctx.view,
                resolve_target: ctx.resolve_target,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            }],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &ctx.renderer.depth_stencil,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &ctx.renderer.config.bind_group, &[]);
        rpass.set_bind_group(1, &self.bind_group, &[]);

        rpass.draw(0..3, 0..1);
        drop(rpass);

        ctx.encoder.pop_debug_group();
    }
}
