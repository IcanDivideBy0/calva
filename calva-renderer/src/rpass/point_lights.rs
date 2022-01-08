use wgpu::util::DeviceExt;

use crate::{util::icosphere::Icosphere, PointLight, RenderContext, Renderer};

pub struct PointLights {
    draw_indirect: wgpu::util::DrawIndexedIndirect,
    draw_indirect_buffer: wgpu::Buffer,

    instances_buffer: wgpu::Buffer,

    stencil_render_bundle: wgpu::RenderBundle,
    lighting_render_bundle: wgpu::RenderBundle,
}

impl PointLights {
    pub const MAX_LIGHTS: usize = 10_000;

    pub fn new(
        renderer: &Renderer,
        albedo_metallic: &wgpu::TextureView,
        normal_roughness: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) -> Self {
        let Renderer {
            device,
            surface_config,
            camera,
            ..
        } = renderer;

        let icosphere = Icosphere::new(1);

        let vertices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PointLights mesh vertices buffer"),
            contents: bytemuck::cast_slice(&icosphere.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PointLights mesh indices buffer"),
            contents: bytemuck::cast_slice(&icosphere.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let vertex_count = icosphere.count;

        let vertex_buffers_layout = [
            PointLight::DESC,
            wgpu::VertexBufferLayout {
                array_stride: (std::mem::size_of::<f32>() * 3) as _,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![3 => Float32x3],
            },
        ];

        let draw_indirect = wgpu::util::DrawIndexedIndirect {
            vertex_count,
            ..Default::default()
        };

        let draw_indirect_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PointLights draw indirect buffer"),
            contents: draw_indirect.as_bytes(),
            usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
        });

        let instances_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PointLights instances buffer"),
            contents: bytemuck::cast_slice(&[PointLight::default(); Self::MAX_LIGHTS]),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("PointLights shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/point_lights.wgsl").into()),
        });

        let stencil_render_bundle = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("PointLights stencil pipeline layout"),
                bind_group_layouts: &[&camera.bind_group_layout],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("PointLights stencil pipeline"),
                layout: Some(&pipeline_layout),
                multiview: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main_stencil",
                    buffers: &vertex_buffers_layout,
                },
                fragment: None,
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Renderer::DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState {
                        front: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::Always,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::DecrementWrap,
                            pass_op: wgpu::StencilOperation::Keep,
                        },
                        back: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::Always,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::IncrementWrap,
                            pass_op: wgpu::StencilOperation::Keep,
                        },
                        read_mask: 0,
                        write_mask: 0xFF,
                    },
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: Renderer::MULTISAMPLE_STATE,
            });

            let mut encoder =
                device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                    label: Some("PointLights stencil render bundle encoder"),
                    color_formats: &[],
                    depth_stencil: Some(wgpu::RenderBundleDepthStencil {
                        format: Renderer::DEPTH_FORMAT,
                        depth_read_only: true,
                        stencil_read_only: false,
                    }),
                    sample_count: Renderer::MULTISAMPLE_STATE.count,
                    multiview: None,
                });

            encoder.set_pipeline(&pipeline);
            encoder.set_bind_group(0, &camera.bind_group, &[]);

            encoder.set_vertex_buffer(0, instances_buffer.slice(..));
            encoder.set_vertex_buffer(1, vertices_buffer.slice(..));
            encoder.set_index_buffer(indices_buffer.slice(..), wgpu::IndexFormat::Uint16);

            encoder.draw_indexed_indirect(&draw_indirect_buffer, 0);

            encoder.finish(&wgpu::RenderBundleDescriptor {
                label: Some("PointLights stencil render bundle"),
            })
        };

        let lighting_render_bundle = {
            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("PointLights lighting bind group layout"),
                    entries: &[
                        // albedo + metallic
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: Renderer::MULTISAMPLE_STATE.count > 1,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            },
                            count: None,
                        },
                        // normal + roughness
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: Renderer::MULTISAMPLE_STATE.count > 1,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            },
                            count: None,
                        },
                        // depth
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: Renderer::MULTISAMPLE_STATE.count > 1,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Depth,
                            },
                            count: None,
                        },
                    ],
                });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("PointLights lighting bind group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(albedo_metallic),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(normal_roughness),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(depth),
                    },
                ],
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("PointLights lighting pipeline layout"),
                bind_group_layouts: &[&camera.bind_group_layout, &bind_group_layout],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("PointLights lighting pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main_lighting",
                    buffers: &vertex_buffers_layout,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main_lighting",
                    targets: &[wgpu::ColorTargetState {
                        format: surface_config.format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Max,
                            },
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: Some(wgpu::Face::Front),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Renderer::DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Always,
                    stencil: wgpu::StencilState {
                        front: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::NotEqual,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::Keep,
                            pass_op: wgpu::StencilOperation::Keep,
                        },
                        back: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::NotEqual,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::Keep,
                            pass_op: wgpu::StencilOperation::Keep,
                        },
                        read_mask: 0xFF,
                        write_mask: 0,
                    },
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: Renderer::MULTISAMPLE_STATE,
                multiview: None,
            });

            let mut encoder =
                device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                    label: Some("PointLights lighting render bundle encoder"),
                    color_formats: &[surface_config.format],
                    depth_stencil: Some(wgpu::RenderBundleDepthStencil {
                        format: Renderer::DEPTH_FORMAT,
                        depth_read_only: true,
                        stencil_read_only: true,
                    }),
                    sample_count: Renderer::MULTISAMPLE_STATE.count,
                    multiview: None,
                });

            encoder.set_pipeline(&pipeline);
            encoder.set_bind_group(0, &camera.bind_group, &[]);
            encoder.set_bind_group(1, &bind_group, &[]);

            encoder.set_vertex_buffer(0, instances_buffer.slice(..));
            encoder.set_vertex_buffer(1, vertices_buffer.slice(..));
            encoder.set_index_buffer(indices_buffer.slice(..), wgpu::IndexFormat::Uint16);

            encoder.draw_indexed_indirect(&draw_indirect_buffer, 0);

            encoder.finish(&wgpu::RenderBundleDescriptor {
                label: Some("PointLights lighting render bundle"),
            })
        };

        Self {
            draw_indirect,
            draw_indirect_buffer,
            instances_buffer,

            stencil_render_bundle,
            lighting_render_bundle,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext, lights: &[PointLight]) {
        ctx.encoder.push_debug_group("PointLights");

        ctx.renderer.queue.write_buffer(
            &self.draw_indirect_buffer,
            0,
            wgpu::util::DrawIndexedIndirect {
                instance_count: lights.len() as u32,
                ..self.draw_indirect
            }
            .as_bytes(),
        );

        ctx.renderer
            .queue
            .write_buffer(&self.instances_buffer, 0, bytemuck::cast_slice(lights));

        ctx.encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PointLights stencil pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &ctx.renderer.depth_stencil,
                    depth_ops: None,
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: true,
                    }),
                }),
            })
            .execute_bundles(std::iter::once(&self.stencil_render_bundle));

        ctx.encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PointLights lighting pass"),
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
                    depth_ops: None,
                    stencil_ops: None,
                }),
            })
            .execute_bundles(std::iter::once(&self.lighting_render_bundle));

        ctx.encoder.pop_debug_group();
    }
}
