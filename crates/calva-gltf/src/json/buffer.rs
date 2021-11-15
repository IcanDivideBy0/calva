use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Buffer {
    pub byte_length: u32,
    pub uri: Option<String>,

    #[serde(skip)]
    pub(crate) data: Vec<u8>,
}

impl Buffer {
    pub(crate) fn from_data(data: Vec<u8>) -> Self {
        Buffer {
            byte_length: data.len() as u32,
            uri: None,
            data,
        }
    }
}
