use serde::Deserialize;

use super::{Camera, Document, Mesh, NodeExtensions, Skin};

pub type NodeExtras = serde_json::Value;

fn default_translation() -> glam::Vec3 {
    glam::vec3(0.0, 0.0, 0.0)
}

fn default_rotation() -> glam::Quat {
    glam::quat(0.0, 0.0, 0.0, 1.0)
}

fn default_scale() -> glam::Vec3 {
    glam::vec3(1.0, 1.0, 1.0)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub name: String,

    #[serde(default = "default_translation")]
    pub translation: glam::Vec3,
    #[serde(default = "default_rotation")]
    pub rotation: glam::Quat,
    #[serde(default = "default_scale")]
    pub scale: glam::Vec3,

    pub mesh: Option<usize>,
    pub skin: Option<usize>,
    pub camera: Option<usize>,

    #[serde(default)]
    pub children: Vec<usize>,

    pub extensions: Option<NodeExtensions>,

    pub extras: Option<NodeExtras>,
}

impl Node {
    pub fn get_transform(&self) -> glam::Mat4 {
        glam::Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    pub fn mesh<'a: 'b, 'b>(&'a self, doc: &'b Document) -> Option<&'b Mesh> {
        self.mesh.and_then(|id| doc.meshes.get(id))
    }

    pub fn skin<'a: 'b, 'b>(&'a self, doc: &'b Document) -> Option<&'b Skin> {
        self.skin.and_then(|id| doc.skins.get(id))
    }

    pub fn camera<'a: 'b, 'b>(&'a self, doc: &'b Document) -> Option<&'b Camera> {
        self.camera.and_then(|id| doc.cameras.get(id))
    }

    pub fn children<'a: 'b, 'b>(&'a self, doc: &'b Document) -> NodeIterator<'b> {
        NodeIterator {
            doc,
            iter: self.children.iter(),
        }
    }
}

pub struct NodeIterator<'a> {
    pub(crate) doc: &'a Document,
    pub(crate) iter: std::slice::Iter<'a, usize>,
}

impl<'a> Iterator for NodeIterator<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().and_then(|id| self.doc.nodes.get(*id))
    }
}

#[test]
fn children() -> serde_json::Result<()> {
    let doc: Document = serde_json::from_str::<Document>(
        r#"{
        "nodes": [
            { "name" : "node_1", "children": [1] },
            { "name" : "node_2", "children": [2] },
            { "name" : "node_3" }
        ]
    }"#,
    )?;

    let children: Vec<_> = doc.nodes[0].children(&doc).collect();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].name, "node_2");

    let children: Vec<_> = children[0].children(&doc).collect();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].name, "node_3");

    let children: Vec<_> = children[0].children(&doc).collect();
    assert_eq!(children.len(), 0);

    Ok(())
}
