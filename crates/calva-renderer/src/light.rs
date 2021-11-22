use wgpu::util::DeviceExt;

use crate::GeometryBuffer;
use crate::RenderContext;
use crate::Renderer;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointLight {
    pub position: glam::Vec3,
    pub radius: f32,
    pub color: glam::Vec3,
}

pub struct LightsPass {
    instances_buffer: wgpu::Buffer,

    positions_buffer: wgpu::Buffer,
    num_elements: u32,
    indices_buffer: wgpu::Buffer,

    stencil_pipeline: wgpu::RenderPipeline,
    lights_pipeline: wgpu::RenderPipeline,
}

impl LightsPass {
    pub fn new(renderer: &Renderer, gbuffer: &GeometryBuffer) -> Self {
        let Renderer {
            device,
            surface_config,
            camera,
            ..
        } = renderer;

        let icosphere = crate::icosphere::Icosphere::new(1);

        let instances_data = [
            PointLight {
                position: (1.0, 3.0, 1.0).into(),
                radius: 1.0,
                color: (1.0, 0.0, 0.0).into(),
            },
            PointLight {
                position: (1.0, 2.0, 1.0).into(),
                radius: 1.0,
                color: (0.0, 1.0, 0.0).into(),
            },
        ];

        let instances_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lights instances buffer"),
            contents: bytemuck::cast_slice(&instances_data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let positions_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lights positions buffer"),
            contents: bytemuck::cast_slice(&icosphere.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lights indices buffer"),
            contents: bytemuck::cast_slice(&icosphere.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let num_elements = icosphere.indices.len() as u32;

        let stencil_pipeline = {
            let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("Lights stencil shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light_stencil.wgsl").into()),
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Lights stencil pipeline layout"),
                bind_group_layouts: &[&camera.bind_group_layout, &gbuffer.bind_group_layout],
                push_constant_ranges: &[],
            });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Lights stencil pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "main",
                    buffers: &[
                        wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<PointLight>() as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![
                                0 => Float32x3, // Position
                                1 => Float32, // Radius
                                2 => Float32x3, // Color
                            ],
                        },
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 3) as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![3 => Float32x3],
                        },
                    ],
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    clamp_depth: false,
                    // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
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
                    bias: wgpu::DepthBiasState {
                        constant: 0,
                        slope_scale: 0.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
            })
        };

        let lights_pipeline = {
            let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("Lights shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light.wgsl").into()),
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Lights pipeline layout"),
                bind_group_layouts: &[&camera.bind_group_layout, &gbuffer.bind_group_layout],
                push_constant_ranges: &[],
            });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Lights pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "main",
                    buffers: &[
                        wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<PointLight>() as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![
                                0 => Float32x3, // Position
                                1 => Float32, // Radius
                                2 => Float32x3, // Color
                            ],
                        },
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 3) as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![3 => Float32x3],
                        },
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "main",
                    targets: &[wgpu::ColorTargetState {
                        format: surface_config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Front),
                    clamp_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
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
                    bias: wgpu::DepthBiasState {
                        constant: 0,
                        slope_scale: 0.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
            })
        };

        Self {
            instances_buffer,

            positions_buffer,
            num_elements,
            indices_buffer,

            stencil_pipeline,
            lights_pipeline,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext, gbuffer: &GeometryBuffer, lights: &[PointLight]) {
        ctx.renderer
            .queue
            .write_buffer(&self.instances_buffer, 0, bytemuck::cast_slice(lights));

        // Lights stencil
        {
            let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Lights stencil pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &gbuffer.depth,
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
            rpass.set_bind_group(1, &gbuffer.bind_group, &[]);

            rpass.set_vertex_buffer(0, self.instances_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.positions_buffer.slice(..));
            rpass.set_index_buffer(self.indices_buffer.slice(..), wgpu::IndexFormat::Uint16);

            rpass.draw_indexed(0..self.num_elements, 0, 0..2);
        }

        // Lights
        {
            let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Lights pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &ctx.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &gbuffer.depth,
                    // depth_ops: None,
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

            rpass.set_pipeline(&self.lights_pipeline);
            rpass.set_bind_group(0, &ctx.renderer.camera.bind_group, &[]);
            rpass.set_bind_group(1, &gbuffer.bind_group, &[]);

            rpass.set_vertex_buffer(0, self.instances_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.positions_buffer.slice(..));

            rpass.set_index_buffer(self.indices_buffer.slice(..), wgpu::IndexFormat::Uint16);

            rpass.draw_indexed(0..self.num_elements, 0, 0..2);
        }
    }
}
