pub use wgpu;

mod camera;
mod egui;
mod gbuffer;
mod icosphere;
mod renderer;
mod texture;

pub use camera::Camera;
pub use renderer::{Renderable, Renderer};
pub use texture::Texture;

pub mod prelude {
    pub use super::*;
}
