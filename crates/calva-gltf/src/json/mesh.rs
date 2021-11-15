use serde::Deserialize;
use std::collections::{hash_map, HashMap};

use super::{Accessor, Document, Material};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mesh {
    pub name: String,
    pub primitives: Vec<MeshPrimitive>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshPrimitive {
    pub attributes: HashMap<String, usize>,
    pub indices: usize,
    pub material: Option<usize>,
}

impl MeshPrimitive {
    pub fn attributes<'a: 'b, 'b>(&'a self, doc: &'b Document) -> AttributeIterator<'b> {
        AttributeIterator {
            doc,
            iter: self.attributes.iter(),
        }
    }

    pub fn attribute<'a: 'b, 'b>(&'a self, name: &str, doc: &'b Document) -> Option<&'b Accessor> {
        self.attributes
            .get(name)
            .and_then(|id| doc.accessors.get(*id))
    }

    pub fn indices<'a: 'b, 'b>(&'a self, doc: &'b Document) -> &'b Accessor {
        doc.accessors.get(self.indices).unwrap()
    }

    pub fn material<'a: 'b, 'b>(&'a self, doc: &'b Document) -> Option<&'b Material> {
        self.material
            .and_then(|material_id| doc.materials.get(material_id))
    }
}

pub struct AttributeIterator<'a> {
    pub(crate) doc: &'a Document,
    pub(crate) iter: hash_map::Iter<'a, String, usize>,
}

impl<'a> Iterator for AttributeIterator<'a> {
    type Item = (&'a String, &'a Accessor);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter
            .next()
            .and_then(|(name, id)| self.doc.accessors.get(*id).map(|accessor| (name, accessor)))
    }
}
