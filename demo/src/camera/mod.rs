use calva::renderer::Renderer;
use std::time::Duration;
use winit::{event::WindowEvent, window::Window};

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
        let projection = Perspective::new(window.inner_size(), 45.0, 0.1, 140.0);

        Self {
            controller,
            projection,
        }
    }

    pub fn handle_event(&mut self, event: &WindowEvent) -> bool {
        self.controller.handle_event(event)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.projection.resize(width, height)
    }

    pub fn _update(&mut self, renderer: &mut Renderer, dt: Duration) {
        self.controller.update(dt);

        renderer.camera.update(
            &renderer.queue,
            self.controller.transform.inverse(),
            self.projection.into(),
        );
    }
}
