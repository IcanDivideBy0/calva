use serde::{Deserialize, Deserializer};

use super::super::{Document, Node};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub target: ChannelTarget,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelTarget {
    pub node: usize,
    pub path: ChannelTargetPath,
}

impl ChannelTarget {
    pub fn node<'a: 'b, 'b>(&'a self, doc: &'b Document) -> Option<&'b Node> {
        doc.nodes.get(self.node)
    }
}

#[derive(Debug)]
pub enum ChannelTargetPath {
    Translation,
    Rotation,
    Scale,
}

impl<'de> Deserialize<'de> for ChannelTargetPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "translation" => Ok(ChannelTargetPath::Translation),
            "rotation" => Ok(ChannelTargetPath::Rotation),
            "scale" => Ok(ChannelTargetPath::Scale),

            value => Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(value),
                &r#"one of ["translation", "rotation", "scale"]"#,
            )),
        }
    }
}
