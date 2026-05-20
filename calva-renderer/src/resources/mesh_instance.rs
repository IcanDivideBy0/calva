use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

use anyhow::Result;

use crate::{
    util::id_generator::IdGenerator, AnimationState, MaterialHandle, MeshHandle, MeshesManager,
    Resource, ResourcesManager,
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
pub struct MeshInstanceHandle(u16);

impl From<MeshInstanceHandle> for usize {
    fn from(value: MeshInstanceHandle) -> usize {
        value.0 as _
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct MeshInstance {
    pub transform: glam::Mat4,
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
    pub animation: AnimationState,
}

bitflags::bitflags! {
    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct MeshInstanceFlags: u8 {
        const ACTIVE           = 1 << 0;
        const UPDATE_TRANSFORM = 1 << 1;
        const UPDATE_ANIMATION = 1 << 2;
    }
}

impl Default for MeshInstanceFlags {
    fn default() -> Self {
        MeshInstanceFlags::all()
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuMeshInstance {
    padding: u16,
    handle: MeshInstanceHandle,
    mesh: MeshHandle,
    material: MaterialHandle,
    flags: MeshInstanceFlags,
    animation: AnimationState,
    transform: glam::Mat4,
}

impl GpuMeshInstance {
    pub(crate) const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;
}

impl From<(MeshInstanceHandle, MeshInstance)> for GpuMeshInstance {
    fn from((handle, instance): (MeshInstanceHandle, MeshInstance)) -> Self {
        Self {
            padding: 0,
            handle,
            mesh: instance.mesh,
            material: instance.material,
            transform: instance.transform,
            animation: instance.animation,
            ..Default::default()
        }
    }
}

impl Hash for GpuMeshInstance {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.handle.hash(state);
    }
}

impl Eq for GpuMeshInstance {}
impl PartialEq for GpuMeshInstance {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

pub struct MeshInstancesManager {
    resources: ResourcesManager,

    ids: IdGenerator,

    pub(crate) base_instances: wgpu::Buffer,
    pub(crate) instances: wgpu::Buffer,

    updates_data: HashSet<GpuMeshInstance>,
    updates: wgpu::Buffer,
    update_bind_group: wgpu::BindGroup,
    update_pipeline: wgpu::ComputePipeline,
}

impl MeshInstancesManager {
    pub const MAX_INSTANCES: usize = 1 << 16;

    fn new(resources: &ResourcesManager) -> Self {
        let resources = resources.clone();
        let device = resources.read::<wgpu::Device>();

        let base_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MeshInstancesManager base instances"),
            size: std::mem::size_of::<[u32; MeshesManager::MAX_MESHES]>() as _,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MeshInstancesManager instances"),
            size: (std::mem::size_of::<[u32; 4]>()
                + std::mem::size_of::<[MeshInstance; Self::MAX_INSTANCES]>())
                as _,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let updates_data = HashSet::with_capacity(Self::MAX_INSTANCES as _);
        let updates = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MeshInstancesManager updates"),
            size: (std::mem::size_of::<[u32; 4]>()
                + std::mem::size_of::<[MeshInstance; Self::MAX_INSTANCES]>())
                as _,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let update_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("MeshInstancesManager[update] bind group layout"),
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
                                    + GpuMeshInstance::SIZE,
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
                                    + GpuMeshInstance::SIZE,
                            ),
                        },
                        count: None,
                    },
                ],
            });

        let update_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MeshInstancesManager[update] bind group"),
            layout: &update_bind_group_layout,
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
            label: Some("MeshInstancesManager shader"),
            source: wgpu::ShaderSource::Wgsl(wesl::include_wesl!("resources::instances").into()),
        });

        let update_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("MeshInstancesManager[update] pipeline layout"),
                bind_group_layouts: &[Some(&update_bind_group_layout)],
                immediate_size: 0,
            });

        let update_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("MeshInstancesManager[update] pipeline"),
            layout: Some(&update_pipeline_layout),
            module: &shader,
            entry_point: Some("update"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            resources,

            ids: IdGenerator::new(0),

            base_instances,
            instances,

            updates_data,
            updates,
            update_bind_group,
            update_pipeline,
        }
    }

    pub fn count(&self) -> u16 {
        self.ids.count()
    }

    pub fn add(&mut self, instances: &[MeshInstance]) -> Vec<MeshInstanceHandle> {
        instances
            .iter()
            .map(|instance| {
                let handle = MeshInstanceHandle(self.ids.get());

                self.updates_data
                    .replace(GpuMeshInstance::from((handle, *instance)));

                handle
            })
            .collect::<Vec<_>>()
    }

    pub fn remove(&mut self, handles: &[MeshInstanceHandle]) {
        for handle in handles.iter() {
            self.updates_data.replace(GpuMeshInstance {
                handle: *handle,
                flags: !MeshInstanceFlags::ACTIVE,
                ..Default::default()
            });

            self.ids.recycle(handle.0 as _);
        }
    }

    pub fn replace(&mut self, data: &[(MeshInstanceHandle, MeshInstance, MeshInstanceFlags)]) {
        for (handle, instance, flags) in data {
            let mut gpu_instance = GpuMeshInstance {
                flags: *flags | MeshInstanceFlags::ACTIVE,
                ..GpuMeshInstance::from((*handle, *instance))
            };

            if let Some(current) = self.updates_data.get(&gpu_instance) {
                if !flags.contains(MeshInstanceFlags::UPDATE_TRANSFORM) {
                    gpu_instance.transform = current.transform;
                }

                if !flags.contains(MeshInstanceFlags::UPDATE_ANIMATION) {
                    gpu_instance.animation = current.animation;
                }

                gpu_instance.flags |= current.flags;
            }

            self.updates_data.replace(gpu_instance);
        }
    }

    pub fn update(&mut self) -> Result<()> {
        let device = self.resources.read::<wgpu::Device>();
        let queue = self.resources.read::<wgpu::Queue>();
        let mut profiler = self.resources.write::<wgpu_profiler::GpuProfiler>();

        let updates_data = self.updates_data.iter().copied().collect::<Vec<_>>();
        let updates_count = updates_data.len() as u32;

        queue.write_buffer(&self.updates, 0, bytemuck::bytes_of(&updates_count));

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
        // 4f90a9bfd867a82c9a788be95069f52131c102e2

        self.updates_data.clear();

        let mut encoder = device.create_command_encoder(&Default::default());
        let mut scope = profiler.scope("MeshInstancesManager", &mut encoder);

        let mut cpass = scope.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("MeshInstancesManager[update]"),
            ..Default::default()
        });

        const WORKGROUP_SIZE: u32 = 256;
        let updates_workgroups_count = updates_count.div_ceil(WORKGROUP_SIZE);

        cpass.set_pipeline(&self.update_pipeline);
        cpass.set_bind_group(0, &self.update_bind_group, &[]);
        cpass.dispatch_workgroups(updates_workgroups_count, 1, 1);

        drop(cpass);
        drop(scope);

        profiler.resolve_queries(&mut encoder);

        queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }
}

impl Resource for MeshInstancesManager {
    fn instanciate(resources: &ResourcesManager) -> Result<Self> {
        Ok(Self::new(resources))
    }

    fn update(&mut self, _resources: &ResourcesManager) -> Result<()> {
        self.update()
    }
}
