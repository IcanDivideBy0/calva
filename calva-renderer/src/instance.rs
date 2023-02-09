use crate::{
    AnimationId, AnimationState, MaterialId, MeshId, MeshesManager, ProfilerCommandEncoder,
};

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

    anim_bind_group: wgpu::BindGroup,
    anim_pipeline: wgpu::ComputePipeline,
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

        let (anim_bind_group, anim_pipeline) = {
            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("InstancesManager[anim] bind group layout"),
                    entries: &[
                        // Cull instances
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(
                                    std::mem::size_of::<[u32; 4]>() as wgpu::BufferAddress
                                        + Instance::SIZE,
                                ),
                            },
                            count: None,
                        },
                    ],
                });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("InstancesManager[anim] bind group"),
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: instances.as_entire_binding(),
                }],
            });

            let shader = device.create_shader_module(wgpu::include_wgsl!("instance.anim.wgsl"));

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("InstancesManager[anim] pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::COMPUTE,
                    range: 0..(std::mem::size_of::<f32>() as _),
                }],
            });

            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("InstancesManager[anim] pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: "main",
            });

            (bind_group, pipeline)
        };

        Self {
            base_instances_data,
            base_instances,

            instances_data,
            instances,

            anim_bind_group,
            anim_pipeline,
        }
    }

    pub fn add(&mut self, queue: &wgpu::Queue, instances: &[Instance]) {
        self.instances_data.extend(instances);

        let mut min_mesh_index: wgpu::BufferAddress = self.base_instances_data.len() as _;
        for instance in instances {
            let mesh_index: usize = instance.mesh.into();

            for base_instance in self.base_instances_data[(mesh_index + 1)..].iter_mut() {
                *base_instance += 1;
            }

            min_mesh_index = min_mesh_index.min(mesh_index as _);
        }

        let first_instance_index = self.instances_data.len() - instances.len();

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

    pub fn anim(&self, encoder: &mut ProfilerCommandEncoder, dt: &std::time::Duration) {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("InstancesManager[anim]"),
        });

        cpass.set_pipeline(&self.anim_pipeline);
        cpass.set_bind_group(0, &self.anim_bind_group, &[]);
        cpass.set_push_constants(0, bytemuck::bytes_of(&dt.as_secs_f32()));
        cpass.dispatch_workgroups(self.instances_data.len() as _, 1, 1);
    }
}
