use crate::Material;

pub struct MeshPrimitive {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material: usize,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeshUniforms {
    pub transform: glam::Mat4,
}

pub struct Mesh {
    pub primitives: Vec<MeshPrimitive>,
    pub instances: Vec<glam::Mat4>,
    pub instances_buffer: wgpu::Buffer,
}

impl Mesh {
    pub const SIZE: wgpu::BufferAddress = std::mem::size_of::<glam::Mat4>() as wgpu::BufferAddress;
    pub const DESC: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: Self::SIZE,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x4,
            1 => Float32x4,
            2 => Float32x4,
            3 => Float32x4,
        ],
    };

    pub fn update(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.instances_buffer,
            0,
            bytemuck::cast_slice(&self.instances),
        );
    }
}

pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}

impl Model {}
