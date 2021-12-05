use wgpu::util::DeviceExt;

use crate::RenderContext;
use crate::Renderer;

use super::icosphere::Icosphere;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointLight {
    pub position: glam::Vec3,
    pub radius: f32,
    pub color: glam::Vec3,
}

impl PointLight {
    pub const DESC: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as _,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3, // Position
            1 => Float32, // Radius
            2 => Float32x3, // Color
        ],
    };
}

pub struct PointLightsPass {
    icosphere: Icosphere,
    instances_buffer: wgpu::Buffer,

    stencil_pipeline: wgpu::RenderPipeline,

    lighting_bind_group: wgpu::BindGroup,
    lighting_pipeline: wgpu::RenderPipeline,
}

impl PointLightsPass {
    pub const MAX_LIGHTS: usize = 10_000;

    pub fn new(
        renderer: &Renderer,

        albedo_metallic: &wgpu::TextureView,
        normal_roughness: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        ssao: &wgpu::TextureView,
    ) -> Self {
        let icosphere = Icosphere::new(&renderer.device, 1);

        let instances_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("PointLights instances buffer"),
                    contents: bytemuck::cast_slice(&[PointLight::default(); Self::MAX_LIGHTS]),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        let vertex_buffers_layout = [
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<PointLight>() as _,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![
                    0 => Float32x3, // Position
                    1 => Float32, // Radius
                    2 => Float32x3, // Color
                ],
            },
            wgpu::VertexBufferLayout {
                array_stride: (std::mem::size_of::<f32>() * 3) as _,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![3 => Float32x3],
            },
        ];

        let stencil_pipeline = {
            let shader = renderer
                .device
                .create_shader_module(&wgpu::ShaderModuleDescriptor {
                    label: Some("PointLights stencil shader"),
                    source: wgpu::ShaderSource::Wgsl(
                        include_str!("shaders/light_stencil.wgsl").into(),
                    ),
                });

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("PointLights stencil pipeline layout"),
                        bind_group_layouts: &[&renderer.camera.bind_group_layout],
                        push_constant_ranges: &[],
                    });

            renderer
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("PointLights stencil pipeline"),
                    layout: Some(&pipeline_layout),
                    multiview: None,
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "main",
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
                })
        };

        let (lighting_bind_group, lighting_pipeline) = {
            let shader = renderer
                .device
                .create_shader_module(&wgpu::ShaderModuleDescriptor {
                    label: Some("PointLights lighting shader"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light.pbr.wgsl").into()),
                });

            let bind_group_layout =
                renderer
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("PointLights lighting bind group layout"),
                        entries: &[
                            // albedo + metallic
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: Renderer::MULTISAMPLE_STATE.count > 1,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: false,
                                    },
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
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: false,
                                    },
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
                            // ssao
                            wgpu::BindGroupLayoutEntry {
                                binding: 3,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: false,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: false,
                                    },
                                },
                                count: None,
                            },
                        ],
                    });

            let bind_group = renderer
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
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
                            resource: wgpu::BindingResource::TextureView(&depth),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(ssao),
                        },
                    ],
                });

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("PointLights lighting pipeline layout"),
                        bind_group_layouts: &[
                            &renderer.config.bind_group_layout,
                            &renderer.camera.bind_group_layout,
                            &bind_group_layout,
                        ],
                        push_constant_ranges: &[],
                    });

            let pipeline =
                renderer
                    .device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("PointLights lighting pipeline"),
                        layout: Some(&pipeline_layout),
                        multiview: None,
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: "main",
                            buffers: &vertex_buffers_layout,
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: "main",
                            targets: &[wgpu::ColorTargetState {
                                format: renderer.surface_config.format,
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
                    });

            (bind_group, pipeline)
        };

        Self {
            icosphere,
            instances_buffer,

            stencil_pipeline,

            lighting_bind_group,
            lighting_pipeline,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext, lights: &[PointLight]) {
        ctx.renderer
            .queue
            .write_buffer(&self.instances_buffer, 0, bytemuck::cast_slice(lights));

        {
            let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PointLights stencil pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &ctx.renderer.depth_stencil,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: true,
                    }),
                }),
            });

            rpass.set_pipeline(&self.stencil_pipeline);
            rpass.set_bind_group(0, &ctx.renderer.camera.bind_group, &[]);

            rpass.set_vertex_buffer(0, self.instances_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.icosphere.vertices.slice(..));
            rpass.set_index_buffer(self.icosphere.indices.slice(..), wgpu::IndexFormat::Uint16);

            rpass.draw_indexed(0..self.icosphere.count, 0, 0..lights.len() as u32);
        }

        {
            let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    }),
                }),
            });

            rpass.set_pipeline(&self.lighting_pipeline);
            rpass.set_bind_group(0, &ctx.renderer.config.bind_group, &[]);
            rpass.set_bind_group(1, &ctx.renderer.camera.bind_group, &[]);
            rpass.set_bind_group(2, &self.lighting_bind_group, &[]);

            rpass.set_vertex_buffer(0, self.instances_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.icosphere.vertices.slice(..));
            rpass.set_index_buffer(self.icosphere.indices.slice(..), wgpu::IndexFormat::Uint16);

            rpass.draw_indexed(0..self.icosphere.count, 0, 0..lights.len() as u32);
        }
    }
}
