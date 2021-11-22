pub use wgpu;

mod ambient;
mod camera;
mod egui;
mod gbuffer;
mod globals;
mod icosphere;
mod light;
mod renderer;
mod ssao;
mod texture;

pub use crate::ambient::AmbientPass;
pub use crate::camera::Camera;
pub use crate::gbuffer::{DrawModel, GeometryBuffer};
pub use crate::globals::ShaderGlobals;
pub use crate::light::LightsPass;
pub use crate::light::PointLight;
pub use crate::renderer::{RenderContext, Renderer};
pub use crate::ssao::SsaoPass;
pub use crate::texture::Texture;

// TODO: add feature flag
pub use crate::egui::EguiPass;
