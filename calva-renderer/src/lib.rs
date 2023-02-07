#![warn(clippy::all)]

pub use wgpu;

mod animation;
mod camera;
mod engine;
mod instance;
mod light;
mod material;
mod mesh;
mod render;
mod renderer;
mod skin;
mod texture;

pub use animation::*;
pub use camera::*;
pub use engine::*;
pub use instance::*;
pub use light::*;
pub use material::*;
pub use mesh::*;
pub use render::*;
pub use renderer::*;
pub use skin::*;
pub use texture::*;

pub mod util {
    pub mod icosphere;
}
