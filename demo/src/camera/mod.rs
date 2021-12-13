use calva::renderer::Renderer;
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
        let projection = Perspective::new(window.inner_size(), 45.0, 1.0, 40.0);

        Self {
            controller,
            projection,
        }
    }

    pub fn process_event(&mut self, event: &WindowEvent) -> bool {
        self.controller.process_event(event)
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.projection.resize(size)
    }

    pub fn update(&mut self, renderer: &mut Renderer, dt: Duration) {
        self.controller.update(dt);

        renderer.update_camera(self.controller.transform.inverse(), self.projection.into())
    }
}
