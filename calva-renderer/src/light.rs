#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointLight {
    pub position: glam::Vec3,
    pub radius: f32,
    pub color: glam::Vec3,
}

pub struct DirectionalLight {
    pub direction: glam::Vec3,
    pub color: glam::Vec4,
}

impl DirectionalLight {}
