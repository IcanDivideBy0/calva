#![warn(clippy::all)]

pub use wgpu;

mod engine;
mod graph;
mod passes;
mod renderer;
mod ressources;

pub use engine::*;
pub use graph::*;
pub use passes::*;
pub use renderer::*;
pub use ressources::*;

pub mod util {
    pub mod icosphere;
}
