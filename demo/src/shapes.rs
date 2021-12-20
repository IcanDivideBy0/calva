use calva::renderer::{
    wgpu::{self, util::DeviceExt},
    GeometryBuffer, Mesh, MeshInstances, Renderer,
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
    instances: MeshInstances,

    positions_buffer: wgpu::Buffer,
    colors_buffer: wgpu::Buffer,
    indices_buffer: wgpu::Buffer,
    num_elements: u32,

    pipeline: wgpu::RenderPipeline,
}

impl SimpleMesh {
    #[allow(dead_code)]
    pub fn new(
        renderer: &Renderer,
        shape: SimpleShape,
        name: &str,
        transform: glam::Mat4,
        color: glam::Vec3,
    ) -> Self {
        let device = &renderer.device;

        let instances = MeshInstances::new(device, vec![transform]);

        let positions_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("Positions Buffer: {}", name)),
            contents: bytemuck::cast_slice(shape.vertices()),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let colors_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("Positions Buffer: {}", name)),
            contents: bytemuck::cast_slice(
                shape
                    .vertices()
                    .iter()
                    .map(|_| color)
                    .collect::<Vec<_>>()
                    .as_slice(),
            ),
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
                multiview: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[
                        MeshInstances::LAYOUT,
                        // Positions
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 3) as _,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![7 => Float32x3],
                        },
                        // colors
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 3) as _,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![8 => Float32x3],
                        },
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: GeometryBuffer::RENDER_TARGETS,
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    // cull_mode: None,
                    cull_mode: Some(wgpu::Face::Back),
                    unclipped_depth: false,
                    // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Renderer::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: Renderer::MULTISAMPLE_STATE,
            })
        };

        Self {
            instances,

            positions_buffer,
            colors_buffer,
            indices_buffer,
            num_elements,

            pipeline,
        }
    }
}

// impl MeshPrimitive for SimpleMesh {
//     fn vertices(&self) -> &wgpu::Buffer {
//         &self.positions_buffer
//     }

//     fn indices(&self) -> &wgpu::Buffer {
//         &self.indices_buffer
//     }

//     fn num_elements(&self) -> u32 {
//         self.num_elements
//     }
// }

// impl Mesh for SimpleMesh {
//     fn instances(&self) -> &MeshInstances {
//         &self.instances
//     }

//     fn instances_mut(&mut self) -> &mut MeshInstances {
//         &mut self.instances
//     }

//     fn primitives(&self) -> Box<dyn Iterator<Item = &dyn MeshPrimitive> + '_> {
//         Box::new(std::iter::once(self as &dyn MeshPrimitive))
//     }
// }

// impl DrawModel for SimpleMesh {
//     fn meshes(&self) -> Box<dyn Iterator<Item = &dyn Mesh> + '_> {
//         Box::new(std::iter::once(self as &dyn Mesh))
//     }

//     fn meshes_mut(&mut self) -> Box<dyn Iterator<Item = &mut dyn Mesh> + '_> {
//         Box::new(std::iter::once(self as &mut dyn Mesh))
//     }

//     fn draw<'s: 'p, 'r: 'p, 'p>(
//         &'s self,
//         renderer: &'r Renderer,
//         rpass: &mut wgpu::RenderPass<'p>,
//     ) {
//         self.instances
//             .write_buffer(&renderer.queue, &renderer.camera);

//         rpass.set_pipeline(&self.pipeline);

//         rpass.set_bind_group(0, &renderer.camera.bind_group, &[]);
//         rpass.set_vertex_buffer(0, self.instances.buffer.slice(..));
//         rpass.set_vertex_buffer(1, self.positions_buffer.slice(..));
//         rpass.set_vertex_buffer(2, self.colors_buffer.slice(..));
//         rpass.set_index_buffer(self.indices_buffer.slice(..), wgpu::IndexFormat::Uint16);

//         rpass.draw_indexed(0..self.num_elements, 0, 0..1);
//     }
// }
