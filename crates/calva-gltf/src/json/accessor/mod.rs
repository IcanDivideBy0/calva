use super::{BufferView, Document};
use serde::Deserialize;

mod component_type;
mod r#type;

pub use component_type::*;
pub use r#type::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Accessor {
    pub buffer_view: usize,
    #[serde(rename = "type")]
    pub ty: AccessorType,
    pub component_type: AccessorComponentType,
    pub count: i32,
}

impl Accessor {
    pub fn buffer_view<'a: 'b, 'b>(&'a self, doc: &'b Document) -> &'b BufferView {
        doc.buffer_views.get(self.buffer_view).unwrap()
    }
}
