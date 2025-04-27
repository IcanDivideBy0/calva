#![warn(clippy::all)]

pub use wgpu;

#[cfg(feature = "egui")]
pub use egui;

mod engine;
mod passes;
mod renderer;
mod resources;
mod uniform_buffer;

pub use engine::*;
pub use passes::*;
pub use renderer::*;
pub use resources::*;
pub use uniform_buffer::*;

pub mod util {
    pub mod icosphere;
    pub mod id_generator;
}
