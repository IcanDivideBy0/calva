use std::collections::BTreeMap;

use crate::{
    util::id_generator::IdGenerator, AnimationHandle, AnimationState, MaterialHandle, MeshHandle,
    MeshesManager,
};

#[repr(C)]
#[derive(
    Debug, Copy, Clone, Default, PartialEq, Eq, Ord, PartialOrd, bytemuck::Pod, bytemuck::Zeroable,
)]
pub struct InstanceHandle(u16);

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
    handle: InstanceHandle,
    __padding__: u16,
    mesh: MeshHandle,
    material: MaterialHandle,
    deleted: u8,
    animation: AnimationState,
    transform: glam::Mat4,
}
impl GpuInstance {
    pub(crate) const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;

    fn address(handle: &InstanceHandle) -> wgpu::BufferAddress {
        std::mem::size_of::<[u32; 4]>() as wgpu::BufferAddress
            + handle.0 as wgpu::BufferAddress * Self::SIZE
    }
}

impl From<(InstanceHandle, Instance)> for GpuInstance {
    fn from((handle, instance): (InstanceHandle, Instance)) -> Self {
        Self {
            handle,
            mesh: instance.mesh,
            material: instance.material,
            transform: instance.transform,
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

    _pending_updates_data: BTreeMap<InstanceHandle, Instance>,
    _pending_updates: wgpu::Buffer,
}

impl InstancesManager {
    pub const MAX_INSTANCES: usize = 1 << 16;

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

        let _pending_updates_data = BTreeMap::new();
        let _pending_updates = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("InstancesManager pending updates"),
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

            _pending_updates_data,
            _pending_updates,
        }
    }

    pub fn count(&self) -> u16 {
        self.ids.count()
    }

    pub fn add(&mut self, queue: &wgpu::Queue, instances: &[Instance]) -> Vec<InstanceHandle> {
        let handles = instances
            .iter()
            .map(|instance| {
                let handle = InstanceHandle(self.ids.get());

                self.instances_meshes.insert(handle, instance.mesh.into());
                let mesh_index: usize = instance.mesh.into();

                for base_instance in self.base_instances_data[(mesh_index + 1)..].iter_mut() {
                    *base_instance += 1;
                }

                handle
            })
            .collect::<Vec<_>>();

        let mut writes: Vec<(wgpu::BufferAddress, Vec<GpuInstance>)> =
            Vec::with_capacity(handles.len());
        if let Some((handle, instance)) = Option::zip(handles.first(), instances.first()) {
            writes.push((
                GpuInstance::address(handle),
                vec![GpuInstance::from((*handle, *instance))],
            ));
        } else {
            return handles;
        }

        for (idx, pair) in handles.windows(2).enumerate() {
            let prev = pair[0];
            let next = pair[1];

            if next.0 != prev.0 + 1 {
                writes.push((GpuInstance::address(&next), vec![]));
            }

            writes
                .last_mut()
                .unwrap()
                .1
                .push(GpuInstance::from((next, instances[idx + 1])));
        }

        for (address, instances) in writes {
            queue.write_buffer(&self.instances, address, bytemuck::cast_slice(&instances));
        }

        queue.write_buffer(
            &self.instances,
            0,
            bytemuck::bytes_of(&(self.count() as u32)),
        );
        queue.write_buffer(
            &self.base_instances,
            0,
            bytemuck::cast_slice(&self.base_instances_data),
        );

        handles
    }

    pub fn remove(&mut self, queue: &wgpu::Queue, handles: &mut [InstanceHandle]) {
        handles.sort();

        for handle in handles.iter() {
            self.ids.recycle(handle.0 as _);

            if let Some(mesh_index) = self.instances_meshes.get(handle) {
                for base_instance in self.base_instances_data[(mesh_index + 1)..].iter_mut() {
                    *base_instance -= 1;
                }
            }
        }

        let mut writes: Vec<(wgpu::BufferAddress, Vec<GpuInstance>)> = vec![];
        if let Some(handle) = handles.first().copied() {
            writes.push((
                GpuInstance::address(&handle),
                vec![GpuInstance {
                    handle,
                    deleted: 1u8,
                    ..Default::default()
                }],
            ));
        } else {
            return;
        }

        for pair in handles.windows(2) {
            let prev = pair[0];
            let next = pair[1];

            if next.0 != prev.0 + 1 {
                writes.push((GpuInstance::address(&next), vec![]));
            }

            writes.last_mut().unwrap().1.push(GpuInstance {
                deleted: 1u8,
                ..Default::default()
            });
        }

        for (address, instances) in writes {
            queue.write_buffer(&self.instances, address, bytemuck::cast_slice(&instances));
        }

        queue.write_buffer(
            &self.base_instances,
            0,
            bytemuck::cast_slice(&self.base_instances_data),
        );
    }
}

impl From<&wgpu::Device> for InstancesManager {
    fn from(device: &wgpu::Device) -> Self {
        Self::new(device)
    }
}
