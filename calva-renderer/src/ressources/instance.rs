use crate::{AnimationId, AnimationState, MaterialId, MeshId, MeshesManager};

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    pub transform: glam::Mat4,
    pub mesh: MeshId,
    pub material: MaterialId,
    pub animation: AnimationState,
}
impl Instance {
    pub const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;

    pub fn transform(&mut self, transform: glam::Mat4) {
        self.transform = transform * self.transform;
    }

    pub fn animate(&mut self, animation: AnimationId) {
        self.animation = AnimationState {
            animation,
            time: 0.0,
        };
    }
}

pub struct InstancesManager {
    base_instances_data: Vec<u32>,
    pub(crate) base_instances: wgpu::Buffer,

    instances_data: Vec<Instance>,
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

        let instances_data = Vec::with_capacity(Self::MAX_INSTANCES);
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
            base_instances_data,
            base_instances,

            instances_data,
            instances,
        }
    }

    pub fn add(&mut self, queue: &wgpu::Queue, instances: impl IntoIterator<Item = Instance>) {
        let first_instance_index = self.instances_data.len();

        let mut min_mesh_index: wgpu::BufferAddress = self.base_instances_data.len() as _;
        for instance in instances.into_iter() {
            self.instances_data.push(instance);
            let mesh_index: usize = instance.mesh.into();

            for base_instance in self.base_instances_data[(mesh_index + 1)..].iter_mut() {
                *base_instance += 1;
            }

            min_mesh_index = min_mesh_index.min(mesh_index as _);
        }

        queue.write_buffer(
            &self.instances,
            0,
            bytemuck::bytes_of(&(self.instances_data.len() as u32)),
        );
        queue.write_buffer(
            &self.instances,
            std::mem::size_of::<[u32; 4]>() as wgpu::BufferAddress
                + first_instance_index as wgpu::BufferAddress * Instance::SIZE,
            bytemuck::cast_slice(&self.instances_data[first_instance_index..]),
        );
        queue.write_buffer(
            &self.base_instances,
            min_mesh_index * std::mem::size_of::<u32>() as wgpu::BufferAddress,
            bytemuck::cast_slice(&self.base_instances_data[(min_mesh_index as _)..]),
        );
    }

    pub fn count(&self) -> u32 {
        self.instances_data.len() as _
    }
}

impl From<&wgpu::Device> for InstancesManager {
    fn from(device: &wgpu::Device) -> Self {
        Self::new(device)
    }
}
