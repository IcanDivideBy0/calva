use serde::Deserialize;

mod khr_lights_punctual;
mod node_extensions;

pub use khr_lights_punctual::*;
pub use node_extensions::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
    #[serde(rename = "KHR_lights_punctual")]
    pub khr_lights_punctual: Option<KhrLightsPunctual>,
}
