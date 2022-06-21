#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointLight {
    pub position: glam::Vec3,
    pub radius: f32,
    pub color: glam::Vec3,
}

impl PointLight {
    pub(crate) const DESC: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as _,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3, // Position
            1 => Float32,   // Radius
            2 => Float32x3, // Color
        ],
    };
}

pub struct DirectionalLight {
    pub direction: glam::Vec3,
    pub color: glam::Vec4,
}

impl DirectionalLight {}
