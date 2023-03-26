use winit::{event::WindowEvent, window::Window};

mod flying_camera;
mod perspective;
pub use flying_camera::*;
pub use perspective::*;

pub struct MyCamera {
    pub controller: FlyingCamera,
    pub projection: Perspective,
}

impl MyCamera {
    pub fn new(window: &Window) -> Self {
        let controller = FlyingCamera::default();
        let projection = Perspective::new(window.inner_size(), 45.0, 0.1, 380.0);

        Self {
            controller,
            projection,
        }
    }

    pub fn handle_event(&mut self, event: &WindowEvent) -> bool {
        self.controller.handle_event(event)
    }

    pub fn resize(&mut self, size: (u32, u32)) {
        self.projection.resize(size)
    }
}
