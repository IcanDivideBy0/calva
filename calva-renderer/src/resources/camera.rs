use anyhow::Result;

use crate::{Resource, UniformBuffer, UniformData};

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

impl Camera {
    pub fn ray_cast(
        &self,
        screen_pos: glam::Vec2,
        viewport_size: glam::Vec2,
    ) -> (glam::Vec3, glam::Vec3) {
        let inv_view = self.view.inverse();
        let inv_proj = self.proj.inverse();

        let origin = inv_view.col(3).truncate();

        let ndc = glam::vec2(
            (2.0 * screen_pos.x) / viewport_size.x - 1.0,
            1.0 - (2.0 * screen_pos.y) / viewport_size.y,
        );

        let mut ray_eye = inv_proj * glam::vec4(ndc.x, ndc.y, -1.0, 1.0);
        ray_eye.z = -1.0;
        ray_eye.w = 0.0;

        let direction = (inv_view * ray_eye).truncate().normalize();

        (origin, direction)
    }
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
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self(UniformBuffer::new(device, queue, Camera::default()))
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

impl Resource for CameraManager {
    fn instanciate(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self::new(device, queue)
    }

    fn update(&mut self) -> Result<()> {
        self.0.update()
    }
}
