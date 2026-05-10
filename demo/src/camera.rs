use anyhow::Result;
use calva::renderer::{wgpu, Camera, Resource, ResourcesManager};

pub struct PerspectiveCamera {
    pub aspect: f32,
    pub fovy: f32, // rad
    pub znear: f32,
    pub zfar: f32,
}

impl PerspectiveCamera {
    pub fn get_proj(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh(self.fovy, self.aspect, self.znear, self.zfar)
    }
}

impl Resource for PerspectiveCamera {
    fn instanciate(resources: &ResourcesManager) -> Self {
        let surface_config = resources.read::<wgpu::SurfaceConfiguration>();

        Self {
            aspect: surface_config.width as f32 / surface_config.height as f32,
            fovy: 45.0_f32.to_radians(),
            znear: 0.1,
            zfar: 100.0,
        }
    }

    fn update(&mut self, resources: &ResourcesManager) -> Result<()> {
        *self = Self::instanciate(resources);

        resources.write::<Camera>().proj = self.get_proj();

        Ok(())
    }
}
