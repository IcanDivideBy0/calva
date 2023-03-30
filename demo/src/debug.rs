use calva::{
    wgpu::{self, util::DeviceExt},
    RenderContext, Renderer,
};

pub enum DebugShape {
    Cube,
    Sphere,
}

impl DebugShape {
    fn buffers(&self, device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer, u32) {
        match self {
            Self::Cube => {
                #[rustfmt::skip]
                const POSITIONS: [f32; 24] = [
                    -1.0, -1.0,  1.0,
                     1.0, -1.0,  1.0,
                     1.0,  1.0,  1.0,
                    -1.0,  1.0,  1.0,
                    -1.0, -1.0, -1.0,
                     1.0, -1.0, -1.0,
                     1.0,  1.0, -1.0,
                    -1.0,  1.0, -1.0,
                ];

                #[rustfmt::skip]
                const INDICES: [u16; 36] = [
                    0, 1, 2,
                    2, 3, 0,
                    1, 5, 6,
                    6, 2, 1,
                    7, 6, 5,
                    5, 4, 7,
                    4, 0, 3,
                    3, 7, 4,
                    4, 5, 1,
                    1, 0, 4,
                    3, 2, 6,
                    6, 7, 3,
                ];

                let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Debug[Cube] vertices"),
                    contents: bytemuck::cast_slice(&POSITIONS),
                    usage: wgpu::BufferUsages::VERTEX,
                });

                let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Debug[Cube] indices"),
                    contents: bytemuck::cast_slice(&INDICES),
                    usage: wgpu::BufferUsages::INDEX,
                });

                (vertices, indices, INDICES.len() as _)
            }
            Self::Sphere => {
                let icosphere = calva::util::icosphere::Icosphere::new(2);

                let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Debug[Sphere] vertices"),
                    contents: bytemuck::cast_slice(&icosphere.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

                let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Debug[Sphere] indices"),
                    contents: bytemuck::cast_slice(&icosphere.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

                (vertices, indices, icosphere.indices.len() as _)
            }
        }
    }
}

pub struct Debug {
    cube: (wgpu::Buffer, wgpu::Buffer, u32),
    sphere: (wgpu::Buffer, wgpu::Buffer, u32),
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

impl Debug {
    pub fn new(renderer: &Renderer) -> Self {
        let cube = DebugShape::Cube.buffers(&renderer.device);
        let sphere = DebugShape::Sphere.buffers(&renderer.device);

        let bind_group_layout =
            renderer
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Debug bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(
                                std::mem::size_of::<glam::Mat4>() as _,
                            ),
                        },
                        count: None,
                    }],
                });

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Debug pipeline layout"),
                    bind_group_layouts: &[&renderer.camera.bind_group_layout, &bind_group_layout],
                    push_constant_ranges: &[wgpu::PushConstantRange {
                        stages: wgpu::ShaderStages::FRAGMENT,
                        range: 0..(std::mem::size_of::<[f32; 4]>() as _),
                    }],
                });

        let shader = renderer
            .device
            .create_shader_module(wgpu::include_wgsl!("debug.wgsl"));

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Debug render pipeline"),
                layout: Some(&pipeline_layout),
                multiview: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<[f32; 3]>() as _,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: Renderer::OUTPUT_FORMAT,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent::OVER,
                            alpha: wgpu::BlendComponent::OVER,
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
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Renderer::MULTISAMPLE_STATE,
            });

        Self {
            cube,
            sphere,
            bind_group_layout,
            pipeline,
        }
    }

    pub fn uniforms(&self, device: &wgpu::Device) -> (wgpu::Buffer, wgpu::BindGroup) {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Debug uniforms"),
            size: std::mem::size_of::<glam::Mat4>() as _,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Debug bind group"),
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        (buffer, bind_group)
    }

    pub fn render(
        &self,
        ctx: &mut RenderContext,
        uniforms: &wgpu::BindGroup,
        shape: DebugShape,
        color: &glam::Vec4,
    ) {
        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Debug"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.output.view,
                resolve_target: ctx.output.resolve_target,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: ctx.output.depth_stencil,
                depth_ops: None,
                stencil_ops: None,
            }),
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &ctx.camera.bind_group, &[]);
        rpass.set_bind_group(1, uniforms, &[]);

        rpass.set_push_constants(wgpu::ShaderStages::FRAGMENT, 0, bytemuck::bytes_of(color));

        let (vertices, indices, count) = match shape {
            DebugShape::Cube => &self.cube,
            DebugShape::Sphere => &self.sphere,
        };

        rpass.set_vertex_buffer(0, vertices.slice(..));
        rpass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint16);

        rpass.draw_indexed(0..*count, 0, 0..1);
    }
}
