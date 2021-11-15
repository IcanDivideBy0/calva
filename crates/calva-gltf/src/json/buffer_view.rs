use byteorder::{ByteOrder, LE};
use serde::Deserialize;
use std::mem::size_of;

use super::{Buffer, Document};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferView {
    pub buffer: usize,
    pub byte_length: usize,
    pub byte_offset: usize,
}

impl BufferView {
    pub fn buffer<'a: 'b, 'b>(&'a self, doc: &'b Document) -> &'b Buffer {
        doc.buffers.get(self.buffer).unwrap()
    }

    pub fn data<'a: 'b, 'b>(&'a self, doc: &'b Document) -> &'b [u8] {
        let start = self.byte_offset;
        let end = start + self.byte_length;

        &self.buffer(doc).data[start..end]
    }

    pub fn data_u16(&self, doc: &Document) -> Vec<u16> {
        let data = self.data(doc);

        let mut result = vec![0u16; data.len() / size_of::<u16>()];
        LE::read_u16_into(&data, &mut result);

        result
    }

    pub fn data_f32(&self, doc: &Document) -> Vec<f32> {
        let data = self.data(doc);

        let mut result = vec![0.0f32; data.len() / size_of::<f32>()];
        LE::read_f32_into(&data, &mut result);

        result
    }

    pub fn data_vec3_array(&self, doc: &Document) -> Vec<glam::Vec3> {
        self.data_f32(doc)
            .chunks_exact(3)
            .map(glam::Vec3::from_slice)
            .collect()
    }

    pub fn data_quat_array(&self, doc: &Document) -> Vec<glam::Quat> {
        self.data_f32(doc)
            .chunks_exact(4)
            .map(glam::Quat::from_slice)
            .collect()
    }

    pub fn data_mat4_array(&self, doc: &Document) -> Vec<glam::Mat4> {
        self.data_f32(doc)
            .chunks_exact(16)
            .map(glam::Mat4::from_cols_slice)
            .collect()
    }
}
