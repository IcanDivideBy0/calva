use crate::Instance;

#[derive(Debug)]
pub struct Mesh {
    pub vertices: wgpu::Buffer,
    pub normals: wgpu::Buffer,
    pub tangents: wgpu::Buffer,
    pub uv0: wgpu::Buffer,
    pub indices: wgpu::Buffer,

    pub num_elements: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshInstance {
    model: [f32; 16],
    normal: [f32; 9],
}

impl From<&glam::Mat4> for MeshInstance {
    fn from(transform: &glam::Mat4) -> Self {
        Self {
            model: transform.to_cols_array(),
            normal: glam::Mat3::from_mat4(transform.inverse().transpose()).to_cols_array(),
        }
    }
}

impl From<&MeshInstance> for glam::Mat4 {
    fn from(instance: &MeshInstance) -> Self {
        glam::Mat4::from_cols_array(&instance.model)
    }
}

impl Instance for MeshInstance {
    const SIZE: usize = std::mem::size_of::<Self>();

    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: Self::SIZE as _,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            // Model matrix
            0 => Float32x4,
            1 => Float32x4,
            2 => Float32x4,
            3 => Float32x4,

            // Normal matrix
            4 => Float32x3,
            5 => Float32x3,
            6 => Float32x3,
        ],
    };
}

pub type MeshInstances = crate::Instances<MeshInstance>;
