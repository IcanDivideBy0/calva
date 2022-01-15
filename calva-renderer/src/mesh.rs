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
    model: glam::Mat4,
    normal: glam::Quat,
}

impl From<&glam::Mat4> for MeshInstance {
    fn from(transform: &glam::Mat4) -> Self {
        let normal_matrix = glam::Mat3::from_mat4(transform.inverse().transpose());

        Self {
            model: *transform,
            normal: glam::Quat::from_mat3(&normal_matrix).normalize(),
        }
    }
}

impl From<&MeshInstance> for glam::Mat4 {
    fn from(instance: &MeshInstance) -> Self {
        instance.model
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
            // Normal quaternion
            4 => Float32x4,
        ],
    };
}

pub type MeshInstances = crate::Instances<MeshInstance>;
