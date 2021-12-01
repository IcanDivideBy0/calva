use crate::GeometryBuffer;
use crate::RenderContext;
use crate::RendererConfig;
use crate::SsaoPass;

pub struct AmbientPass {
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl AmbientPass {
    pub fn new(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        config: &RendererConfig,
        gbuffer: &GeometryBuffer,
        ssao: &SsaoPass,
    ) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("AmbientPass bind group layout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("AmbientPass bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&ssao.output),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("AmbientPass pipeline layout"),
            bind_group_layouts: &[
                &config.bind_group_layout,
                &gbuffer.bind_group_layout,
                &bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("AmbientPass shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ambient.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("AmbientPass pipeline"),
            layout: Some(&pipeline_layout),
            multiview: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "main",
                targets: &[wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        });

        Self {
            bind_group,
            pipeline,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("AmbientPass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &ctx.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &ctx.renderer.config.bind_group, &[]);
        rpass.set_bind_group(1, &ctx.renderer.gbuffer.bind_group, &[]);
        rpass.set_bind_group(2, &self.bind_group, &[]);

        rpass.draw(0..6, 0..1);
    }
}
