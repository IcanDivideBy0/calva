use crate::{AnimationState, MaterialId, MeshId, MeshInfo, MeshesManager, ProfilerCommandEncoder};

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
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CulledInstance {
    _model_matrix: [f32; 16],
    _normal_quat: [f32; 4],
    _material: MaterialId,
    _skin_offset: i32,
    _animation: AnimationState,
}
impl CulledInstance {
    pub const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;

    pub(crate) const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: Self::SIZE,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            // Model matrix
            0 => Float32x4,
            1 => Float32x4,
            2 => Float32x4,
            3 => Float32x4,
            // Normal quat
            4 => Float32x4,
            // Material
            5 => Uint32,

            // Skinning
            6 => Sint32, // Skin offset
            7 => Uint32, // Animation ID
            8 => Float32, // Animation time
        ],
    };
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawIndexedIndirect {
    vertex_count: u32,
    instance_count: u32,
    base_index: u32,
    vertex_offset: i32,
    base_instance: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct Frustum(pub [glam::Vec4; 6]);

impl Frustum {
    const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;
}

impl From<&glam::Mat4> for Frustum {
    fn from(value: &glam::Mat4) -> Self {
        use glam::Vec4Swizzles;

        let l = value.row(3) + value.row(0); // left
        let r = value.row(3) - value.row(0); // right
        let b = value.row(3) + value.row(1); // bottom
        let t = value.row(3) - value.row(1); // top
        let n = value.row(3) + value.row(2); // near
        let f = value.row(3) - value.row(2); // far

        Self([
            l / l.xyz().length(),
            r / r.xyz().length(),
            b / b.xyz().length(),
            t / t.xyz().length(),
            n / n.xyz().length(),
            f / f.xyz().length(),
        ])
    }
}

pub struct InstancesManager {
    frustum: wgpu::Buffer,
    base_instances_data: Vec<u32>,
    base_instances: wgpu::Buffer,

    instances_data: Vec<Instance>,
    instances: wgpu::Buffer,

    pub(crate) culled_instances: wgpu::Buffer,
    pub(crate) indirect_draws: wgpu::Buffer,

    cull_bind_group: wgpu::BindGroup,
    cull_reset_pipeline: wgpu::ComputePipeline,
    cull_pipeline: wgpu::ComputePipeline,
    cull_count_pipeline: wgpu::ComputePipeline,
}

impl InstancesManager {
    const MAX_INSTANCES: usize = 1_000_000;

    pub fn new(device: &wgpu::Device, meshes: &MeshesManager) -> Self {
        let frustum = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("InstancesManager frustum data"),
            size: Frustum::SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let base_instances_data = Vec::with_capacity(MeshesManager::MAX_MESHES);
        let base_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("InstancesManager base instances"),
            size: std::mem::size_of::<[u32; MeshesManager::MAX_MESHES]>() as _,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instances_data = Vec::with_capacity(Self::MAX_INSTANCES);
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("InstancesManager instances"),
            size: std::mem::size_of::<[Instance; Self::MAX_INSTANCES]>() as _,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let culled_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("InstancesManager culled instances"),
            size: (std::mem::size_of::<[CulledInstance; Self::MAX_INSTANCES]>()) as _,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let indirect_draws = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("InstancesManager indirect draws"),
            size: (std::mem::size_of::<u32>()
                + std::mem::size_of::<[DrawIndexedIndirect; MeshesManager::MAX_MESHES]>())
                as _,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::INDIRECT,
            mapped_at_creation: false,
        });

        let (cull_bind_group, cull_reset_pipeline, cull_pipeline, cull_count_pipeline) = {
            let shader = device.create_shader_module(wgpu::include_wgsl!("instance.cull.wgsl"));

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("InstancesManager[cull] bind group layout"),
                    entries: &[
                        // Frustum
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(Frustum::SIZE),
                            },
                            count: None,
                        },
                        // Mesh data
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(MeshInfo::SIZE),
                            },
                            count: None,
                        },
                        // Base instances
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(
                                    std::mem::size_of::<u32>() as _
                                ),
                            },
                            count: None,
                        },
                        // Mesh instances
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(Instance::SIZE),
                            },
                            count: None,
                        },
                        // Culled instances
                        wgpu::BindGroupLayoutEntry {
                            binding: 4,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(CulledInstance::SIZE),
                            },
                            count: None,
                        },
                        // Indirect draws
                        wgpu::BindGroupLayoutEntry {
                            binding: 5,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(
                                    std::mem::size_of::<u32>() as u64
                                        + std::mem::size_of::<DrawIndexedIndirect>() as u64,
                                ),
                            },
                            count: None,
                        },
                    ],
                });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("InstancesManager[cull] bind group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: frustum.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: meshes.meshes_info.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: base_instances.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: instances.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: culled_instances.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: indirect_draws.as_entire_binding(),
                    },
                ],
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("InstancesManager[cull] pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::COMPUTE,
                    range: 0..(std::mem::size_of::<u32>() as _),
                }],
            });

            let reset_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("InstancesManager[cull] reset pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: "reset",
            });

            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("InstancesManager[cull] pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: "cull",
            });

            let count_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("InstancesManager[cull] count pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: "count",
            });

            (bind_group, reset_pipeline, pipeline, count_pipeline)
        };

        Self {
            frustum,

            base_instances_data,
            base_instances,

            instances_data,
            instances,

            culled_instances,
            indirect_draws,

            cull_bind_group,
            cull_reset_pipeline,
            cull_pipeline,
            cull_count_pipeline,
        }
    }

    pub fn add(&mut self, queue: &wgpu::Queue, meshes: &MeshesManager, instance: Instance) {
        self.base_instances_data
            .resize(meshes.count() as _, self.instances_data.len() as _);

        let mesh_index: usize = instance.mesh.into();

        let base_instance = self.base_instances_data[mesh_index];
        self.instances_data.insert(base_instance as _, instance);

        for base_instance in self.base_instances_data[(mesh_index + 1)..].iter_mut() {
            *base_instance += 1;
        }

        queue.write_buffer(
            &self.base_instances,
            0,
            bytemuck::cast_slice(&self.base_instances_data),
        );
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Instance> {
        self.instances_data.iter_mut()
    }

    pub(crate) fn cull(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut ProfilerCommandEncoder,
        view_proj: &glam::Mat4,
    ) {
        queue.write_buffer(
            &self.frustum,
            0,
            bytemuck::bytes_of(&Frustum::from(view_proj)),
        );

        queue.write_buffer(
            &self.instances,
            0,
            bytemuck::cast_slice(&self.instances_data),
        );

        queue.write_buffer(&self.indirect_draws, 0, bytemuck::bytes_of(&0_u32));

        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("InstancesManager[cull]"),
        });

        let meshes_count: u32 = self.base_instances_data.len() as _;
        let instances_count: u32 = self.instances_data.len() as _;

        cpass.set_pipeline(&self.cull_reset_pipeline);
        cpass.set_bind_group(0, &self.cull_bind_group, &[]);
        cpass.dispatch_workgroups(meshes_count, 1, 1);

        cpass.set_pipeline(&self.cull_pipeline);
        cpass.set_bind_group(0, &self.cull_bind_group, &[]);
        cpass.set_push_constants(0, bytemuck::bytes_of(&instances_count));
        cpass.dispatch_workgroups(instances_count, 1, 1);

        cpass.set_pipeline(&self.cull_count_pipeline);
        cpass.dispatch_workgroups(meshes_count, 1, 1);
    }
}
