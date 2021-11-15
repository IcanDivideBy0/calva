use serde::Deserialize;

mod channel;
mod sampler;

pub use channel::*;
pub use sampler::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Animation {
    pub name: String,
    pub channels: Vec<Channel>,
    pub samplers: Vec<Sampler>,
}
