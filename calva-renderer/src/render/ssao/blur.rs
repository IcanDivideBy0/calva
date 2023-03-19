use crate::{RenderContext, Renderer};

use super::SsaoPass;

#[derive(Clone, Copy)]
enum Direction {
    Horizontal,
    Vertical,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Direction::Horizontal => "horizontal",
            Direction::Vertical => "vertical",
        })
    }
}

pub struct SsaoBlur<const WIDTH: u32, const HEIGHT: u32> {
    temp: wgpu::TextureView,

    h_pass: wgpu::RenderBundle,
    v_pass: wgpu::RenderBundle,
}

impl<const WIDTH: u32, const HEIGHT: u32> SsaoBlur<WIDTH, HEIGHT> {
    pub fn new(renderer: &Renderer, output: &wgpu::TextureView) -> Self {
        let temp = SsaoPass::<WIDTH, HEIGHT>::make_texture(renderer, Some("SsaoBlur temp texture"));

        let bind_group_layout =
            renderer
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("SsaoBlur bind group layout"),
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

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("SsaoBlur pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let shader = renderer
            .device
            .create_shader_module(wgpu::include_wgsl!("blur.wgsl"));

        let make_render_bundle = |direction: Direction| {
            let bind_group = renderer
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(format!("SsaoBlur[{direction}] bind group").as_str()),
                    layout: &bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(match direction {
                            Direction::Horizontal => output,
                            Direction::Vertical => &temp,
                        }),
                    }],
                });

            let pipeline =
                renderer
                    .device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some(format!("SsaoBlur[{direction}] pipeline").as_str()),
                        layout: Some(&pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: "vs_main",
                            buffers: &[],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: format!("fs_main_{direction}").as_str(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format: SsaoPass::<WIDTH, HEIGHT>::OUTPUT_FORMAT,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        primitive: Default::default(),
                        depth_stencil: None,
                        multisample: Default::default(),
                        multiview: None,
                    });

            let mut encoder = renderer.device.create_render_bundle_encoder(
                &wgpu::RenderBundleEncoderDescriptor {
                    label: Some(format!("SsaoBlur[{direction}] render bundle").as_str()),
                    color_formats: &[Some(SsaoPass::<WIDTH, HEIGHT>::OUTPUT_FORMAT)],
                    depth_stencil: None,
                    sample_count: 1,
                    multiview: None,
                },
            );

            encoder.set_pipeline(&pipeline);
            encoder.set_bind_group(0, &bind_group, &[]);

            encoder.draw(0..3, 0..1);

            encoder.finish(&Default::default())
        };

        let h_pass = make_render_bundle(Direction::Horizontal);
        let v_pass = make_render_bundle(Direction::Vertical);

        Self {
            temp,

            h_pass,
            v_pass,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext, output: &wgpu::TextureView) {
        #[cfg(feature = "profiler")]
        ctx.encoder.profile_start("Ssao[blur]");

        ctx.encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Ssao[blur][horizontal]"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.temp,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            })
            .execute_bundles(std::iter::once(&self.h_pass));

        ctx.encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Ssao[blur][vertical]"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            })
            .execute_bundles(std::iter::once(&self.v_pass));

        #[cfg(feature = "profiler")]
        ctx.encoder.profile_end();
    }
}
