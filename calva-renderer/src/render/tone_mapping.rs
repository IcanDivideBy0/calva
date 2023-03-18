use crate::{RenderContext, Renderer};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ToneMappingConfig {
    pub exposure: f32,
    pub gamma: f32,
}

impl Default for ToneMappingConfig {
    fn default() -> Self {
        Self {
            exposure: 1.0,
            gamma: 2.2,
        }
    }
}

pub struct ToneMappingPass {
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl ToneMappingPass {
    pub fn new(renderer: &Renderer) -> Self {
        let bind_group_layout =
            renderer
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let bind_group = Self::make_bind_group(renderer, &bind_group_layout);

        let shader = renderer
            .device
            .create_shader_module(wgpu::include_wgsl!("tone_mapping.wgsl"));

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("ToneMapping pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[wgpu::PushConstantRange {
                        stages: wgpu::ShaderStages::FRAGMENT,
                        range: 0..(std::mem::size_of::<ToneMappingConfig>() as _),
                    }],
                });

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                        format: renderer.surface_config.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
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

    pub fn render(&self, ctx: &mut RenderContext, config: &ToneMappingConfig) {
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
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_push_constants(wgpu::ShaderStages::FRAGMENT, 0, bytemuck::bytes_of(config));

        rpass.draw(0..3, 0..1);
    }

    fn make_bind_group(renderer: &Renderer, layout: &wgpu::BindGroupLayout) -> wgpu::BindGroup {
        renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ToneMapping bind group"),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&renderer.output),
                }],
            })
    }
}
