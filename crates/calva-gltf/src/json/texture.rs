use serde::Deserialize;

use super::{Document, Image};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Texture {
    pub source: usize,
}

impl Texture {
    pub fn source<'a: 'b, 'b>(&'a self, doc: &'b Document) -> &'b Image {
        doc.images.get(self.source).unwrap()
    }
}
