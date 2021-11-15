use serde::Deserialize;

use super::{Document, NodeIterator};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub name: String,
    pub nodes: Vec<usize>,
}

impl Scene {
    pub fn nodes<'a: 'b, 'b>(&'a self, doc: &'b Document) -> NodeIterator<'b> {
        NodeIterator {
            doc,
            iter: self.nodes.iter(),
        }
    }
}
