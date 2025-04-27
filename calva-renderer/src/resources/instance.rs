use std::collections::BTreeMap;

use crate::{
    util::id_generator::IdGenerator, AnimationHandle, AnimationState, MaterialHandle, MeshHandle,
    MeshesManager,
};

#[repr(C)]
#[derive(
    Debug, Copy, Clone, Default, PartialEq, Eq, Ord, PartialOrd, bytemuck::Pod, bytemuck::Zeroable,
)]
pub struct InstanceHandle(u32);

impl From<InstanceHandle> for u32 {
    fn from(value: InstanceHandle) -> u32 {
        value.0
    }
}
impl From<InstanceHandle> for usize {
    fn from(value: InstanceHandle) -> usize {
        value.0 as _
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct Instance {
    pub transform: glam::Mat4,
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
    pub animation: AnimationState,
}
impl Instance {
    pub fn transform(&mut self, transform: glam::Mat4) {
        self.transform = transform * self.transform;
    }

    pub fn animate(&mut self, animation: AnimationHandle) {
        self.animation = AnimationState {
            animation,
            time: 0.0,
        };
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuInstance {
    transform: glam::Mat4,
    mesh: MeshHandle,
    material: MaterialHandle,
    deleted: u8,
    __padding__: u32,
    animation: AnimationState,
}
impl GpuInstance {
    pub(crate) const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;

    fn address(handle: &InstanceHandle) -> wgpu::BufferAddress {
        std::mem::size_of::<[u32; 4]>() as wgpu::BufferAddress
            + handle.0 as wgpu::BufferAddress * Self::SIZE
    }
}

impl From<Instance> for GpuInstance {
    fn from(instance: Instance) -> Self {
        Self {
            transform: instance.transform,
            mesh: instance.mesh,
            material: instance.material,
            animation: instance.animation,
            ..Default::default()
        }
    }
}

pub struct InstancesManager {
    ids: IdGenerator,

    base_instances_data: Vec<u32>,
    pub(crate) base_instances: wgpu::Buffer,

    instances_meshes: BTreeMap<InstanceHandle, usize>,
    pub(crate) instances: wgpu::Buffer,
}

impl InstancesManager {
    pub const MAX_INSTANCES: usize = 1_000_000;

    pub fn new(device: &wgpu::Device) -> Self {
        let base_instances_data = vec![0; MeshesManager::MAX_MESHES];
        let base_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("InstancesManager base instances"),
            size: std::mem::size_of::<[u32; MeshesManager::MAX_MESHES]>() as _,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instances_meshes = BTreeMap::new();
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("InstancesManager instances"),
            size: (std::mem::size_of::<[u32; 4]>()
                + std::mem::size_of::<[Instance; Self::MAX_INSTANCES]>()) as _,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        Self {
            ids: IdGenerator::new(0),

            base_instances_data,
            base_instances,

            instances_meshes,
            instances,
        }
    }

    pub fn add(&mut self, queue: &wgpu::Queue, instances: &[Instance]) -> Vec<InstanceHandle> {
        let handles = instances
            .iter()
            .copied()
            .map(|instance| {
                let handle = InstanceHandle(self.ids.get());

                self.instances_meshes.insert(handle, instance.mesh.into());
                let mesh_index: usize = instance.mesh.into();

                for base_instance in self.base_instances_data[(mesh_index + 1)..].iter_mut() {
                    *base_instance += 1;
                }

                queue.write_buffer(
                    &self.instances,
                    GpuInstance::address(&handle),
                    bytemuck::bytes_of(&GpuInstance::from(instance)),
                );

                handle
            })
            .collect::<Vec<_>>();

        queue.write_buffer(
            &self.instances,
            0,
            bytemuck::bytes_of(&(self.instances_meshes.len() as u32)),
        );
        queue.write_buffer(
            &self.base_instances,
            0,
            bytemuck::cast_slice(&self.base_instances_data),
        );

        handles
    }

    pub fn remove(&mut self, queue: &wgpu::Queue, handles: &[InstanceHandle]) {
        for handle in handles {
            self.ids.recycle(handle.0);

            if let Some(mesh_index) = self.instances_meshes.get(handle) {
                for base_instance in self.base_instances_data[(mesh_index + 1)..].iter_mut() {
                    *base_instance -= 1;
                }

                queue.write_buffer(
                    &self.instances,
                    GpuInstance::address(handle),
                    bytemuck::bytes_of(&GpuInstance {
                        deleted: 1u8,
                        ..Default::default()
                    }),
                );
            }
        }

        queue.write_buffer(
            &self.base_instances,
            0,
            bytemuck::cast_slice(&self.base_instances_data),
        );
    }

    pub fn count(&self) -> u32 {
        self.instances_meshes.len() as _
    }
}

impl From<&wgpu::Device> for InstancesManager {
    fn from(device: &wgpu::Device) -> Self {
        Self::new(device)
    }
}
