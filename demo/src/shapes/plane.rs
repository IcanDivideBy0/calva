use calva::{
    prelude::*,
    renderer::wgpu::{self, util::DeviceExt},
};

#[rustfmt::skip]
pub const VERTICES : [[f32; 3]; 4] = [
    [ 1.0,  1.0,  0.0],
    [-1.0, -1.0,  0.0],
    [ 1.0, -1.0,  0.0],
    [-1.0,  1.0,  0.0],
];

pub const INDICES: [u16; 6] = [0, 1, 2, 0, 3, 1];

#[allow(dead_code)]
pub fn build_model(renderer: &Renderer, name: &str) -> Model {
    let primitive = {
        let vertex_buffer = renderer
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Vertex Buffer: {}", name)),
                contents: bytemuck::cast_slice(&VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer = renderer
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Index Buffer: {}", name)),
                contents: bytemuck::cast_slice(&INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

        MeshPrimitive {
            vertex_buffer,
            index_buffer,
            num_elements: INDICES.len() as u32,
            material: 0,
        }
    };

    let mesh = {
        let instances = vec![glam::Mat4::default()];
        let instances_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Mesh Transform Buffer: {}", name)),
                    contents: bytemuck::cast_slice(&instances),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        Mesh {
            primitives: vec![primitive],
            instances,
            instances_buffer,
        }
    };

    let material = { Material::new(renderer, name) };

    Model {
        meshes: vec![mesh],
        materials: vec![material],
    }
}
