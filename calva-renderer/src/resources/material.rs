use crate::{util::id_generator::IdGenerator, Resource, ResourcesManager, TextureHandle};
use anyhow::Result;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialHandle(u8);

#[repr(C)]
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Material {
    pub albedo: TextureHandle,
    pub normal: TextureHandle,
    pub metallic_roughness: TextureHandle,
    pub emissive: TextureHandle,
}

impl Material {
    pub const SIZE: wgpu::BufferAddress = std::mem::size_of::<Material>() as _;

    fn address(handle: &MaterialHandle) -> wgpu::BufferAddress {
        handle.0 as wgpu::BufferAddress * Self::SIZE
    }
}

pub struct MaterialsManager {
    resources: ResourcesManager,

    ids: IdGenerator,
    buffer: wgpu::Buffer,

    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,
}

impl MaterialsManager {
    const MAX_MATERIALS: usize = 1 << 8; // see material_id

    fn new(resources: &ResourcesManager) -> Self {
        let resources = resources.clone();
        let device = resources.read::<wgpu::Device>();

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MaterialManager buffer"),
            contents: bytemuck::cast_slice(&[Material::default(); Self::MAX_MATERIALS]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MaterialManager bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Material>() as _),
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialManager bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            resources,

            ids: IdGenerator::new(1),
            buffer,

            bind_group_layout,
            bind_group,
        }
    }

    pub fn add(&mut self, materials: &[Material]) -> Vec<MaterialHandle> {
        let queue = self.resources.read::<wgpu::Queue>();

        materials
            .iter()
            .map(|material| {
                let handle = MaterialHandle(self.ids.get() as u8);

                queue.write_buffer(
                    &self.buffer,
                    Material::address(&handle),
                    bytemuck::bytes_of(material),
                );

                handle
            })
            .collect::<Vec<_>>()
    }
}

impl Resource for MaterialsManager {
    fn instanciate(resources: &ResourcesManager) -> Result<Self> {
        Ok(Self::new(resources))
    }
}
