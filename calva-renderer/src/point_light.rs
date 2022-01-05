use wgpu::util::DeviceExt;

use crate::{util::icosphere::Icosphere, RenderContext, Renderer};

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

struct PointLightMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    num_elements: u32,
}

impl PointLightMesh {
    pub fn new(device: &wgpu::Device) -> Self {
        let icosphere = Icosphere::new(1);

        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PointLightMesh vertices buffer"),
            contents: bytemuck::cast_slice(&icosphere.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PointLightMesh indices buffer"),
            contents: bytemuck::cast_slice(&icosphere.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertices,
            indices,
            num_elements: icosphere.count,
        }
    }
}

pub struct PointLights {
    instances_buffer: wgpu::Buffer,
    mesh: PointLightMesh,

    stencil_pipeline: wgpu::RenderPipeline,

    lighting_bind_group: wgpu::BindGroup,
    lighting_pipeline: wgpu::RenderPipeline,
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

        let instances_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PointLights instances buffer"),
            contents: bytemuck::cast_slice(&[PointLight::default(); Self::MAX_LIGHTS]),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let mesh = PointLightMesh::new(device);

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
            let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("PointLights stencil shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light_stencil.wgsl").into()),
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("PointLights stencil pipeline layout"),
                bind_group_layouts: &[&camera.bind_group_layout],
                push_constant_ranges: &[],
            });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("PointLights stencil pipeline"),
                layout: Some(&pipeline_layout),
                multiview: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
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
            let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("PointLights lighting shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light.pbr.wgsl").into()),
            });

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
                multiview: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &vertex_buffers_layout,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
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
            });

            (bind_group, pipeline)
        };

        Self {
            instances_buffer,
            mesh,

            stencil_pipeline,

            lighting_bind_group,
            lighting_pipeline,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext, lights: &[PointLight]) {
        ctx.encoder.push_debug_group("PointLights");

        ctx.renderer
            .queue
            .write_buffer(&self.instances_buffer, 0, bytemuck::cast_slice(lights));

        // Stencil pass
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
            rpass.set_vertex_buffer(1, self.mesh.vertices.slice(..));
            rpass.set_index_buffer(self.mesh.indices.slice(..), wgpu::IndexFormat::Uint16);

            rpass.draw_indexed(0..self.mesh.num_elements, 0, 0..lights.len() as u32);
        }

        // lighting pass
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
            rpass.set_bind_group(0, &ctx.renderer.camera.bind_group, &[]);
            rpass.set_bind_group(1, &self.lighting_bind_group, &[]);

            rpass.set_vertex_buffer(0, self.instances_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.mesh.vertices.slice(..));
            rpass.set_index_buffer(self.mesh.indices.slice(..), wgpu::IndexFormat::Uint16);

            rpass.draw_indexed(0..self.mesh.num_elements, 0, 0..lights.len() as u32);
        }

        ctx.encoder.pop_debug_group();
    }
}