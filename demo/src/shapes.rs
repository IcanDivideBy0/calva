use calva::renderer::{
    wgpu::{self, util::DeviceExt},
    DrawModel, Renderer,
};

mod plane {
    #[rustfmt::skip]
    pub const VERTICES : [[f32; 3]; 4] = [
        [ 1.0,  1.0,  0.0],
        [-1.0, -1.0,  0.0],
        [ 1.0, -1.0,  0.0],
        [-1.0,  1.0,  0.0],
    ];

    pub const INDICES: [u16; 6] = [0, 1, 2, 0, 3, 1];
}

mod cube {
    #[allow(dead_code)]
    #[rustfmt::skip]
    pub const VERTICES: [[f32; 3]; 8] = [
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
    pub const INDICES: [u16; 36] = [
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
}

#[allow(dead_code)]
pub enum SimpleShape {
    Plane,
    Cube,
}

impl SimpleShape {
    fn vertices(&self) -> &[[f32; 3]] {
        match self {
            Self::Plane => &plane::VERTICES,
            Self::Cube => &cube::VERTICES,
        }
    }

    fn indices(&self) -> &[u16] {
        match self {
            Self::Plane => &plane::INDICES,
            Self::Cube => &cube::INDICES,
        }
    }
}

pub struct SimpleMesh {
    positions_buffer: wgpu::Buffer,
    indices_buffer: wgpu::Buffer,
    num_elements: u32,
    pipeline: wgpu::RenderPipeline,
}

impl SimpleMesh {
    #[allow(dead_code)]
    pub fn new(renderer: &Renderer, shape: SimpleShape, name: &str) -> Self {
        let device = &renderer.device;

        let positions_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("Positions Buffer: {}", name)),
            contents: bytemuck::cast_slice(shape.vertices()),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("Index Buffer: {}", name)),
            contents: bytemuck::cast_slice(shape.indices()),
            usage: wgpu::BufferUsages::INDEX,
        });

        let num_elements = shape.indices().len() as u32;

        let pipeline = {
            let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some(&format!("Shader: {}", name)),
                source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&format!("Render Pipeline Layout: {}", name)),
                bind_group_layouts: &[&renderer.camera.bind_group_layout],
                push_constant_ranges: &[],
            });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("Render Pipeline: {}", name)),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "main",
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: (std::mem::size_of::<glam::Vec3>()) as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "main",
                    targets: Renderer::RENDER_TARGETS,
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    clamp_depth: false,
                    // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(Renderer::DEPTH_STENCIL),
                multisample: Renderer::MULTISAMPLE,
            })
        };

        Self {
            positions_buffer,
            indices_buffer,
            num_elements,
            pipeline,
        }
    }
}

impl DrawModel for SimpleMesh {
    fn draw<'ctx: 'pass, 'pass>(
        &'ctx self,
        renderer: &'ctx Renderer,
        rpass: &mut wgpu::RenderPass<'pass>,
    ) {
        rpass.set_pipeline(&self.pipeline);

        rpass.set_bind_group(0, &renderer.camera.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.positions_buffer.slice(..));
        rpass.set_index_buffer(self.indices_buffer.slice(..), wgpu::IndexFormat::Uint16);

        rpass.draw_indexed(0..self.num_elements, 0, 0..1);
    }
}
