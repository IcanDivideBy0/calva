use std::time::Duration;

use calva::renderer::Camera;
use winit::{dpi::PhysicalSize, event::WindowEvent};

mod flying_camera;
mod perspective;
pub use flying_camera::*;
pub use perspective::*;

pub struct MyCamera {
    pub aspect: f32,
    pub fovy: f32, // rad
    pub znear: f32,
    pub zfar: f32,

    pub controller: FlyingCamera,
}

impl MyCamera {
    pub fn new(size: PhysicalSize<u32>) -> Self {
        Self {
            aspect: size.width as f32 / size.height as f32,
            fovy: 45.0_f32.to_radians(),
            znear: 0.1,
            zfar: 380.0,

            controller: FlyingCamera::default(),
        }
    }

    pub fn handle_event(&mut self, event: &WindowEvent) -> bool {
        self.controller.handle_event(event)
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.aspect = size.width as f32 / size.height as f32;
    }

    pub fn update(&mut self, dt: Duration) {
        self.controller.update(dt);
    }
}

impl From<&MyCamera> for Camera {
    fn from(camera: &MyCamera) -> Camera {
        Camera {
            view: camera.controller.transform.inverse(),
            proj: glam::Mat4::perspective_rh(camera.fovy, camera.aspect, camera.znear, camera.zfar),
        }
    }
}
