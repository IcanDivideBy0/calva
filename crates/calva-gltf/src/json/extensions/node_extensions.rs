use serde::Deserialize;

use super::NodeKhrLightsPunctual;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeExtensions {
    #[serde(rename = "KHR_lights_punctual")]
    pub khr_lights_punctual: Option<NodeKhrLightsPunctual>,
}
