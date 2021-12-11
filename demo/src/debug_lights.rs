use calva::renderer::{
    wgpu::{self, util::DeviceExt},
    PointLight, PointLights, RenderContext, Renderer,
};

#[allow(dead_code)]
#[rustfmt::skip]
const VERTICES: [[f32; 3]; 8] = [
    [-1.0, -1.0,  1.0],
    [ 1.0, -1.0,  1.0],
    [ 1.0,  1.0,  1.0],
    [-1.0,  1.0,  1.0],
    [-1.0, -1.0, -1.0],
    [ 1.0, -1.0, -1.0],
    [ 1.0,  1.0, -1.0],
    [-1.0,  1.0, -1.0],
];

#[allow(dead_code)]
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

struct Cube {
    positions: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
}

impl Cube {
    fn new(device: &wgpu::Device) -> Self {
        let positions = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("DebugLights:Cube positions buffer"),
            contents: bytemuck::cast_slice(&VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("DebugLights:Cube indices buffer"),
            contents: bytemuck::cast_slice(&INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            positions,
            indices,
            count: INDICES.len() as u32,
        }
    }
}

pub struct DebugLights {
    cube: Cube,
    instances_buffer: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
}

impl DebugLights {
    pub fn new(renderer: &Renderer) -> Self {
        let cube = Cube::new(&renderer.device);

        let instances_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("DebugLights:Cube instances buffer"),
                    contents: bytemuck::cast_slice(
                        &[PointLight::default(); PointLights::MAX_LIGHTS],
                    ),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        let shader = renderer
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("DebugLights shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/debug_lights.wgsl").into()),
            });

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("DebugLights pipeline layout"),
                    bind_group_layouts: &[&renderer.camera.bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("DebugLights pipeline"),
                layout: Some(&pipeline_layout),
                multiview: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[
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
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[wgpu::ColorTargetState {
                        format: renderer.surface_config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Renderer::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: Renderer::MULTISAMPLE_STATE,
            });

        Self {
            cube,
            instances_buffer,
            pipeline,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext, lights: &[PointLight]) {
        ctx.encoder.push_debug_group("DebugLights");

        ctx.queue
            .write_buffer(&self.instances_buffer, 0, bytemuck::cast_slice(lights));

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("DebugLights pass"),
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
                    load: wgpu::LoadOp::Clear(0),
                    store: true,
                }),
            }),
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &ctx.renderer.camera.bind_group, &[]);

        rpass.set_vertex_buffer(0, self.instances_buffer.slice(..));
        rpass.set_vertex_buffer(1, self.cube.positions.slice(..));
        rpass.set_index_buffer(self.cube.indices.slice(..), wgpu::IndexFormat::Uint16);

        rpass.draw_indexed(0..self.cube.count, 0, 0..lights.len() as u32);
        drop(rpass);

        ctx.encoder.pop_debug_group();
    }
}
