use crate::{PointLight, RenderContext, Renderer};

pub struct LightsPass {
    point_lights_pass: PointLightsPass,
}

impl LightsPass {
    pub fn new(
        renderer: &Renderer,
        albedo_metallic: &wgpu::TextureView,
        normal_roughness: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) -> Self {
        Self {
            point_lights_pass: PointLightsPass::new(
                renderer,
                albedo_metallic,
                normal_roughness,
                depth,
            ),
        }
    }

    pub fn resize(
        &mut self,
        renderer: &Renderer,
        albedo_metallic: &wgpu::TextureView,
        normal_roughness: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) {
        self.point_lights_pass
            .resize(renderer, albedo_metallic, normal_roughness, depth);
    }

    pub fn render(&self, ctx: &mut RenderContext, gamma: f32, point_lights: &[PointLight]) {
        self.point_lights_pass.render(ctx, gamma, point_lights);
    }
}
use point_lights::*;
mod point_lights {
    use wgpu::util::DeviceExt;

    use crate::{util::icosphere::Icosphere, PointLight, RenderContext, Renderer};

    pub struct PointLightsPass {
        vertex_count: u32,
        vertices: wgpu::Buffer,
        indices: wgpu::Buffer,
        instances: wgpu::Buffer,

        bind_group_layout: wgpu::BindGroupLayout,
        bind_group: wgpu::BindGroup,

        stencil_pipeline: wgpu::RenderPipeline,
        lighting_pipeline: wgpu::RenderPipeline,
    }

    impl PointLightsPass {
        pub const MAX_LIGHTS: usize = 10_000;

        pub fn new(
            renderer: &Renderer,
            albedo_metallic: &wgpu::TextureView,
            normal_roughness: &wgpu::TextureView,
            depth: &wgpu::TextureView,
        ) -> Self {
            let icosphere = Icosphere::new(1);

            let vertices = renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("PointLights mesh vertices buffer"),
                    contents: bytemuck::cast_slice(&icosphere.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

            let indices = renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("PointLights mesh indices buffer"),
                    contents: bytemuck::cast_slice(&icosphere.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

            let instances = renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("PointLights instances buffer"),
                    contents: bytemuck::cast_slice(&[PointLight::default(); Self::MAX_LIGHTS]),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

            let vertex_buffers_layout = [
                // PointLights instances
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Self>() as _,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3, // Position
                        1 => Float32,   // Radius
                        2 => Float32x3, // Color
                    ],
                },
                // Icosphere vertices
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<[f32; 3]>() as _,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![3 => Float32x3],
                },
            ];

            let shader = renderer
                .device
                .create_shader_module(wgpu::include_wgsl!("shaders/lights.point-lights.wgsl"));

            let stencil_pipeline = {
                let pipeline_layout =
                    renderer
                        .device
                        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                            label: Some("PointLights[stencil] pipeline layout"),
                            bind_group_layouts: &[&renderer.camera.bind_group_layout],
                            push_constant_ranges: &[],
                        });

                renderer
                    .device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("PointLights[stencil] pipeline"),
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
                    })
            };

            let bind_group_layout =
                renderer
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("PointLights[lighting] bind group layout"),
                        entries: &[
                            // albedo + metallic
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
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
                            // normal + roughness
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
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

            let bind_group = Self::make_bind_group(
                &renderer.device,
                &bind_group_layout,
                albedo_metallic,
                normal_roughness,
                depth,
            );

            let lighting_pipeline = {
                let pipeline_layout =
                    renderer
                        .device
                        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                            label: Some("PointLights[lighting] pipeline layout"),
                            bind_group_layouts: &[
                                &renderer.camera.bind_group_layout,
                                &bind_group_layout,
                            ],
                            push_constant_ranges: &[wgpu::PushConstantRange {
                                stages: wgpu::ShaderStages::FRAGMENT,
                                range: 0..(std::mem::size_of::<f32>() as _),
                            }],
                        });

                renderer
                    .device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("PointLights[lighting] pipeline"),
                        layout: Some(&pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: "vs_main_lighting",
                            buffers: &vertex_buffers_layout,
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: "fs_main_lighting",
                            targets: &[Some(wgpu::ColorTargetState {
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
                            })],
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
                    })
            };

            Self {
                vertex_count: icosphere.count,
                vertices,
                indices,
                instances,

                bind_group_layout,
                bind_group,

                stencil_pipeline,
                lighting_pipeline,
            }
        }

        pub fn resize(
            &mut self,
            renderer: &Renderer,
            albedo_metallic: &wgpu::TextureView,
            normal_roughness: &wgpu::TextureView,
            depth: &wgpu::TextureView,
        ) {
            self.bind_group = Self::make_bind_group(
                &renderer.device,
                &self.bind_group_layout,
                albedo_metallic,
                normal_roughness,
                depth,
            );
        }

        pub fn render(&self, ctx: &mut RenderContext, gamma: f32, point_lights: &[PointLight]) {
            ctx.encoder.profile_start("PointLights");

            ctx.queue
                .write_buffer(&self.instances, 0, bytemuck::cast_slice(point_lights));

            let mut stencil_pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PointLights[stencil]"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &ctx.output.depth_stencil,
                    depth_ops: None,
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: true,
                    }),
                }),
            });

            stencil_pass.set_pipeline(&self.stencil_pipeline);
            stencil_pass.set_bind_group(0, &ctx.camera.bind_group, &[]);

            stencil_pass.set_vertex_buffer(0, self.instances.slice(..));
            stencil_pass.set_vertex_buffer(1, self.vertices.slice(..));
            stencil_pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);

            stencil_pass.draw_indexed(0..self.vertex_count, 0, 0..(point_lights.len() as _));

            drop(stencil_pass);

            let mut lighting_pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PointLights[lighting]"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: ctx.output.view,
                    resolve_target: ctx.output.resolve_target,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &ctx.output.depth_stencil,
                    depth_ops: None,
                    stencil_ops: None,
                }),
            });

            lighting_pass.set_pipeline(&self.lighting_pipeline);
            lighting_pass.set_bind_group(0, &ctx.camera.bind_group, &[]);
            lighting_pass.set_bind_group(1, &self.bind_group, &[]);
            lighting_pass.set_push_constants(
                wgpu::ShaderStages::FRAGMENT,
                0,
                bytemuck::bytes_of(&gamma),
            );

            lighting_pass.set_vertex_buffer(0, self.instances.slice(..));
            lighting_pass.set_vertex_buffer(1, self.vertices.slice(..));
            lighting_pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);

            lighting_pass.draw_indexed(0..self.vertex_count, 0, 0..(point_lights.len() as _));

            drop(lighting_pass);

            ctx.encoder.profile_end();
        }

        fn make_bind_group(
            device: &wgpu::Device,
            layout: &wgpu::BindGroupLayout,
            albedo_metallic: &wgpu::TextureView,
            normal_roughness: &wgpu::TextureView,
            depth: &wgpu::TextureView,
        ) -> wgpu::BindGroup {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("PointLights[lighting] bind group"),
                layout,
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
            })
        }
    }
}
