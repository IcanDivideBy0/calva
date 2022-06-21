#![warn(clippy::all)]

pub use wgpu;

mod camera;
mod config;
mod instance;
mod light;
mod material;
mod mesh;
mod renderer;
mod skin;

pub mod graph;

pub use camera::CameraUniform;
pub use config::{RendererConfig, RendererConfigData};
pub use instance::{Instance, Instances};
pub use light::{DirectionalLight, PointLight};
pub use material::Material;
pub use mesh::{Mesh, MeshInstance, MeshInstances};
pub use renderer::{RenderContext, Renderer};
pub use skin::{
    Skin, SkinAnimation, SkinAnimationFrame, SkinAnimationInstance, SkinAnimationInstances,
    SkinAnimations,
};

pub mod util {
    pub mod icosphere;
    pub mod mipmap;
}
