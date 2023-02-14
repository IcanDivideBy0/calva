use crate::{RenderContext, Renderer};

pub struct FxaaPass {
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl FxaaPass {
    pub fn new(renderer: &Renderer) -> Self {
        let bind_group_layout =
            renderer
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("FXAA bind group layout"),
                    entries: &[
                        // input
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

        let bind_group = Self::make_bind_group(renderer, &bind_group_layout);

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("FXAA pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[wgpu::PushConstantRange {
                        stages: wgpu::ShaderStages::FRAGMENT,
                        range: 0..(std::mem::size_of::<f32>() as _),
                    }],
                });

        let shader = renderer
            .device
            .create_shader_module(wgpu::include_wgsl!("fxaa.wgsl"));

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("FXAA pipeline"),
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
                        format: renderer.surface_config.format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
            });

        Self {
            bind_group_layout,
            bind_group,
            pipeline,
        }
    }

    pub fn rebind(&mut self, renderer: &Renderer) {
        self.bind_group = Self::make_bind_group(renderer, &self.bind_group_layout);
    }

    pub fn render(&self, ctx: &mut RenderContext, gamma: f32) {
        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("AmbientLight"),
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
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_push_constants(wgpu::ShaderStages::FRAGMENT, 0, bytemuck::bytes_of(&gamma));

        rpass.draw(0..3, 0..1);
    }

    fn make_bind_group(renderer: &Renderer, layout: &wgpu::BindGroupLayout) -> wgpu::BindGroup {
        renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("FXAA bind group"),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&renderer.output),
                }],
            })
    }
}
