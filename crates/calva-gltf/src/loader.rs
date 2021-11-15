use anyhow::{anyhow, Result};
use calva_renderer::{
    prelude::*,
    wgpu::{self, util::DeviceExt},
};
use std::io::Read;

use crate::json;

pub fn load(mut reader: &mut dyn Read, renderer: &Renderer) -> Result<Model> {
    let doc = json::Document::try_from_reader(&mut reader)?;

    let device = &renderer.device;

    let meshes = doc
        .meshes
        .iter()
        .map(|mesh| {
            let make_primitive = |primitive: &json::MeshPrimitive| -> Result<MeshPrimitive> {
                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Vertex Buffer: {}", &mesh.name)),
                    contents: primitive
                        .attribute("POSITION", &doc)
                        .ok_or(anyhow!("No positions buffer"))?
                        .buffer_view(&doc)
                        .data(&doc),
                    usage: wgpu::BufferUsages::VERTEX,
                });

                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Index Buffer: {}", &mesh.name)),
                    contents: primitive.indices(&doc).buffer_view(&doc).data(&doc),
                    usage: wgpu::BufferUsages::INDEX,
                });

                Ok(MeshPrimitive {
                    vertex_buffer,
                    index_buffer,
                    num_elements: primitive.indices(&doc).count as u32,
                    material: primitive.material.ok_or(anyhow!("No material"))?,
                })
            };

            let instances = vec![glam::Mat4::default()];
            let instances_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Mesh Transform Buffer: {}", mesh.name)),
                contents: bytemuck::cast_slice(&instances),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

            Ok(Mesh {
                primitives: mesh
                    .primitives
                    .iter()
                    .map(make_primitive)
                    .collect::<Result<_>>()?,
                instances,
                instances_buffer,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let materials = doc
        .materials
        .iter()
        .map(|material| Material::new(renderer, &material.name))
        .collect();

    Ok(Model { meshes, materials })
}
