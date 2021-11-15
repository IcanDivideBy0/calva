use anyhow::{anyhow, bail, Result};
use byteorder::{ByteOrder, LE};
use serde::Deserialize;
use std::{io::Read, mem::size_of};

use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    #[serde(default)]
    pub scene: usize,

    #[serde(default)]
    pub accessors: Vec<Accessor>,

    #[serde(default)]
    pub animations: Vec<Animation>,

    #[serde(default)]
    pub buffer_views: Vec<BufferView>,

    #[serde(default)]
    pub buffers: Vec<Buffer>,

    #[serde(default)]
    pub cameras: Vec<Camera>,

    #[serde(default)]
    pub images: Vec<Image>,

    #[serde(default)]
    pub materials: Vec<Material>,

    #[serde(default)]
    pub meshes: Vec<Mesh>,

    #[serde(default)]
    pub nodes: Vec<Node>,

    #[serde(default)]
    pub scenes: Vec<Scene>,

    #[serde(default)]
    pub skins: Vec<Skin>,

    #[serde(default)]
    pub textures: Vec<Texture>,

    pub extensions: Option<Extensions>,
}

impl Document {
    pub fn try_from_reader(reader: &mut dyn Read) -> Result<Self> {
        // Parsing header
        {
            const SIZE: usize = size_of::<u32>() * 3;
            let mut buf: [u8; SIZE] = [0; SIZE];
            let mut handle = reader.take(SIZE as u64);
            handle.read_exact(&mut buf)?;
            let mut header: [u32; 3] = [0; 3];
            LE::read_u32_into(&buf, &mut header);

            let [magic, version, _length] = header;

            if std::str::from_utf8(&magic.to_le_bytes())? != "glTF" {
                bail!("Invalid glTF magic header");
            }

            if version != 2 {
                bail!("Only glTF version 2 is supported");
            }
        }

        let mut doc: Option<Document> = None;
        let mut bin_buffers: Vec<Vec<u8>> = vec![];

        for _ in 0..2 {
            const HEADER_SIZE: usize = size_of::<u32>() * 2;
            let mut buf: [u8; HEADER_SIZE] = [0; HEADER_SIZE];
            let mut handle = reader.take(HEADER_SIZE as u64);

            handle.read_exact(&mut buf)?;
            let mut chunk_header: [u32; 2] = [0; 2];
            LE::read_u32_into(&buf, &mut chunk_header);

            let [chunk_length, chunk_type] = chunk_header;
            let mut chunk_data_reader = reader.take(chunk_length as u64);

            match std::str::from_utf8(&chunk_type.to_le_bytes())? {
                "JSON" => {
                    doc = Some(serde_json::from_reader(chunk_data_reader)?);
                }
                "BIN\0" => {
                    let mut bin_buf: Vec<u8> = vec![];
                    chunk_data_reader.read_to_end(&mut bin_buf)?;
                    bin_buffers.push(bin_buf);
                }
                ty => bail!("Unknown chunk type: '{}'", ty),
            };
        }

        let mut doc = doc.ok_or_else(|| anyhow!("No JSON section in file"))?;

        match bin_buffers.len() {
            0 => bail!("No bin section in file"),
            _ => {
                doc.bind_buffers(bin_buffers);
            }
        };

        Ok(doc)
    }

    fn bind_buffers(&mut self, mut buffers: Vec<Vec<u8>>) {
        self.buffers = buffers.drain(..).map(Buffer::from_data).collect();
    }

    pub fn scene(&self) -> &Scene {
        self.scenes.get(self.scene).unwrap()
    }
}
