use serde::Deserialize;

use super::super::super::Document;
use super::Light;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeKhrLightsPunctual {
    #[serde(rename = "light")]
    pub light_index: usize,
}

impl NodeKhrLightsPunctual {
    pub fn light<'a: 'b, 'b>(&'a self, doc: &'b Document) -> &'b Light {
        doc.extensions
            .as_ref()
            .and_then(|extensions| extensions.khr_lights_punctual.as_ref())
            .and_then(|khr_lights_punctual| khr_lights_punctual.lights.get(self.light_index))
            .unwrap()
    }
}
