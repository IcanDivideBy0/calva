use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

use crate::{
    util::id_generator::IdGenerator, AnimationHandle, AnimationState, MaterialHandle, MeshHandle,
    MeshesManager, RenderContext,
};

#[repr(C)]
#[derive(
    Debug,
    Copy,
    Clone,
    Default,
    PartialEq,
    Eq,
    Ord,
    PartialOrd,
    bytemuck::Pod,
    bytemuck::Zeroable,
    Hash,
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
    active: u8,
    animation: AnimationState,
    transform: glam::Mat4,
}

impl GpuInstance {
    pub(crate) const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;
}

impl Hash for GpuInstance {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.handle.hash(state);
    }
}

impl Eq for GpuInstance {}
impl PartialEq for GpuInstance {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl From<(InstanceHandle, Instance)> for GpuInstance {
    fn from((handle, instance): (InstanceHandle, Instance)) -> Self {
        Self {
            handle,
            mesh: instance.mesh,
            material: instance.material,
            active: 1,
            transform: instance.transform,
            animation: instance.animation,
            ..Default::default()
        }
    }
}

pub struct InstancesManager {
    ids: IdGenerator,

    pub(crate) base_instances: wgpu::Buffer,
    pub(crate) instances: wgpu::Buffer,

    updates_data: HashSet<GpuInstance>,
    updates: wgpu::Buffer,
    maintain_bind_group: wgpu::BindGroup,
    maintain_pipeline: wgpu::ComputePipeline,
}

impl InstancesManager {
    pub const MAX_INSTANCES: usize = 1 << 16;

    pub fn new(device: &wgpu::Device) -> Self {
        let base_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("InstancesManager base instances"),
            size: std::mem::size_of::<[u32; MeshesManager::MAX_MESHES]>() as _,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("InstancesManager instances"),
            size: (std::mem::size_of::<[u32; 4]>()
                + std::mem::size_of::<[Instance; Self::MAX_INSTANCES]>()) as _,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let updates_data = HashSet::with_capacity(Self::MAX_INSTANCES as _);
        let updates = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("InstancesManager updates"),
            size: (std::mem::size_of::<[u32; 4]>()
                + std::mem::size_of::<[Instance; Self::MAX_INSTANCES]>()) as _,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let maintain_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("InstancesManager[maintain] bind group layout"),
                entries: &[
                    // Updates
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(
                                std::mem::size_of::<[u32; 4]>() as wgpu::BufferAddress
                                    + GpuInstance::SIZE,
                            ),
                        },
                        count: None,
                    },
                    // Base instances
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(
                                std::mem::size_of::<u32>() as _
                            ),
                        },
                        count: None,
                    },
                    // Instances
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(
                                std::mem::size_of::<[u32; 4]>() as wgpu::BufferAddress
                                    + GpuInstance::SIZE,
                            ),
                        },
                        count: None,
                    },
                ],
            });

        let maintain_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("InstancesManager[maintain] bind group"),
            layout: &maintain_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: updates.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: base_instances.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: instances.as_entire_binding(),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("InstancesManager shader"),
            source: wgpu::ShaderSource::Wgsl(wesl::include_wesl!("resources::instances").into()),
        });

        let maintain_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("InstancesManager[maintain] pipeline layout"),
                bind_group_layouts: &[&maintain_bind_group_layout],
                push_constant_ranges: &[],
            });

        let maintain_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("InstancesManager[maintain] pipeline"),
            layout: Some(&maintain_pipeline_layout),
            module: &shader,
            entry_point: Some("maintain"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            ids: IdGenerator::new(0),

            base_instances,
            instances,

            updates_data,
            updates,
            maintain_bind_group,
            maintain_pipeline,
        }
    }

    pub fn count(&self) -> u16 {
        self.ids.count()
    }

    pub fn add(&mut self, instances: &[Instance]) -> Vec<InstanceHandle> {
        instances
            .iter()
            .map(|instance| {
                let handle = InstanceHandle(self.ids.get());

                self.updates_data
                    .replace(GpuInstance::from((handle, *instance)));

                handle
            })
            .collect::<Vec<_>>()
    }

    pub fn remove(&mut self, handles: &mut [InstanceHandle]) {
        for handle in handles.iter() {
            self.updates_data.replace(GpuInstance {
                handle: *handle,
                ..Default::default()
            });

            self.ids.recycle(handle.0 as _);
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue) {
        let updates_data = self.updates_data.iter().copied().collect::<Vec<_>>();

        queue.write_buffer(
            &self.updates,
            0,
            bytemuck::bytes_of(&(updates_data.len() as u32)),
        );

        queue.write_buffer(
            &self.updates,
            std::mem::size_of::<[u32; 4]>() as wgpu::BufferAddress,
            bytemuck::cast_slice(&updates_data),
        );

        // queue.write_buffer(
        //     &self.base_instances,
        //     0,
        //     bytemuck::cast_slice(&self.base_instances_data),
        // );

        self.updates_data.clear();
    }

    pub fn maintain(&self, ctx: &mut RenderContext) {
        let mut cpass = ctx.encoder.scoped_compute_pass("InstancesManager[update]");

        const WORKGROUP_SIZE: u32 = 32;

        let updates_workgroups_count =
            (Self::MAX_INSTANCES as f32 / WORKGROUP_SIZE as f32).ceil() as u32;

        cpass.set_pipeline(&self.maintain_pipeline);
        cpass.set_bind_group(0, &self.maintain_bind_group, &[]);
        cpass.dispatch_workgroups(updates_workgroups_count, 1, 1);
    }
}

impl From<&wgpu::Device> for InstancesManager {
    fn from(device: &wgpu::Device) -> Self {
        Self::new(device)
    }
}
