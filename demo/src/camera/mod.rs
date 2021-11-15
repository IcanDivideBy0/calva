use calva::prelude::Camera;
use std::time::Duration;
use winit::{dpi::PhysicalSize, event::WindowEvent, window::Window};

mod flying_camera;
mod perspective;
use flying_camera::*;
use perspective::*;

pub struct MyCamera {
    pub controller: FlyingCamera,
    pub projection: Perspective,
}

impl MyCamera {
    pub fn new(window: &Window) -> Self {
        let controller = FlyingCamera::default();
        let projection = Perspective::new(window.inner_size(), 45.0, 0.1, 2000.0);

        Self {
            controller,
            projection,
        }
    }

    pub fn process_event(&mut self, event: &WindowEvent) -> bool {
        self.controller.process_event(event)
    }

    pub fn update(&mut self, dt: Duration) {
        self.controller.update(dt)
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.projection.resize(size)
    }
}

impl Camera for MyCamera {
    fn view(&self) -> glam::Mat4 {
        glam::Mat4::inverse(&self.controller.transform)
    }

    fn proj(&self) -> glam::Mat4 {
        self.projection.into()
    }
}
