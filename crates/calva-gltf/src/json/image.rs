use serde::Deserialize;

use super::{BufferView, Document};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub name: String,
    pub mime_type: String,
    pub buffer_view: usize,
}

impl Image {
    pub fn buffer_view<'a: 'b, 'b>(&'a self, doc: &'b Document) -> &'b BufferView {
        doc.buffer_views.get(self.buffer_view).unwrap()
    }
}
