#![warn(clippy::all)]

pub use wgpu;

mod engine;
mod graph;
mod passes;
mod renderer;
mod ressources;
mod uniform_buffer;

pub use engine::*;
pub use graph::*;
pub use passes::*;
pub use renderer::*;
pub use ressources::*;
pub use uniform_buffer::*;

pub mod util {
    pub mod icosphere;
}
