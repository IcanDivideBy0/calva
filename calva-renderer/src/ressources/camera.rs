use crate::{Ressource, UniformBuffer, UniformData};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuCamera {
    view: glam::Mat4,
    proj: glam::Mat4,
    view_proj: glam::Mat4,
    inv_view: glam::Mat4,
    inv_proj: glam::Mat4,
    frustum: [glam::Vec4; 6],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Camera {
    pub view: glam::Mat4,
    pub proj: glam::Mat4,
}

impl UniformData for Camera {
    type GpuType = GpuCamera;

    fn as_gpu_type(&self) -> Self::GpuType {
        let view_proj = self.proj * self.view;

        let frustum = {
            use glam::Vec4Swizzles;

            let l = view_proj.row(3) + view_proj.row(0); // left
            let r = view_proj.row(3) - view_proj.row(0); // right
            let b = view_proj.row(3) + view_proj.row(1); // bottom
            let t = view_proj.row(3) - view_proj.row(1); // top
            let n = view_proj.row(3) + view_proj.row(2); // near
            let f = view_proj.row(3) - view_proj.row(2); // far

            [
                l / l.xyz().length(),
                r / r.xyz().length(),
                b / b.xyz().length(),
                t / t.xyz().length(),
                n / n.xyz().length(),
                f / f.xyz().length(),
            ]
        };

        GpuCamera {
            view: self.view,
            proj: self.proj,
            view_proj,
            inv_view: self.view.inverse(),
            inv_proj: self.proj.inverse(),
            frustum,
        }
    }
}

pub struct CameraManager(UniformBuffer<Camera>);

impl CameraManager {
    pub fn new(device: &wgpu::Device) -> Self {
        Self(UniformBuffer::new(device, Camera::default()))
    }
}

impl std::ops::Deref for CameraManager {
    type Target = UniformBuffer<Camera>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for CameraManager {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Ressource for CameraManager {
    fn instanciate(device: &wgpu::Device) -> Self {
        Self::new(device)
    }
}
