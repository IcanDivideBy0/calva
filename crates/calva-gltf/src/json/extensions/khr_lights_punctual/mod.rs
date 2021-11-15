use serde::Deserialize;

mod light;
mod node;

pub use light::*;
pub use node::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrLightsPunctual {
    pub lights: Vec<Light>,
}
