pub use wgpu;

mod camera;
mod egui;
mod material;
mod model;
mod renderer;
mod texture;

pub use camera::Camera;
pub use material::Material;
pub use model::{Mesh, MeshPrimitive, Model};
pub use renderer::Renderer;

pub mod prelude {

    pub use super::*;
}
