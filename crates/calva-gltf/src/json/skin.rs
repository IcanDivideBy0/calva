use serde::Deserialize;

use super::{Accessor, Document, NodeIterator};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
    pub name: String,
    pub joints: Vec<usize>,
    pub inverse_bind_matrices: usize,
}

impl Skin {
    pub fn joints<'a: 'b, 'b>(&'a self, doc: &'b Document) -> NodeIterator<'b> {
        NodeIterator {
            doc,
            iter: self.joints.iter(),
        }
    }

    pub fn inverse_bind_matrices<'a: 'b, 'b>(&'a self, doc: &'b Document) -> &'b Accessor {
        doc.accessors.get(self.inverse_bind_matrices).unwrap()
    }
}
