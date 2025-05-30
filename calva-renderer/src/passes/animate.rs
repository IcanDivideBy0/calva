use std::time::Duration;

use crate::{
    GpuInstance, InstancesManager, RenderContext, ResourceRef, ResourcesManager, UniformBuffer,
    UniformData,
};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct AnimateUniform(Duration);

impl std::ops::Deref for AnimateUniform {
    type Target = Duration;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for AnimateUniform {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl UniformData for AnimateUniform {
    type GpuType = f32;

    fn as_gpu_type(&self) -> Self::GpuType {
        self.0.as_secs_f32()
    }
}

pub struct AnimatePass {
    pub uniform: UniformBuffer<AnimateUniform>,

    instances: ResourceRef<InstancesManager>,

    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
}

impl AnimatePass {
    pub fn new(device: &wgpu::Device, resources: &ResourcesManager) -> Self {
        let uniform = UniformBuffer::new(device, AnimateUniform::default());

        let instances = resources.get::<InstancesManager>();

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Animate bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<[u32; 4]>() as wgpu::BufferAddress + GpuInstance::SIZE,
                    ),
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Animate bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: instances.get().instances.as_entire_binding(),
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Animate shader"),
            source: wgpu::ShaderSource::Wgsl(wesl::include_wesl!("passes::animate").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Animate pipeline layout"),
            bind_group_layouts: &[&bind_group_layout, &uniform.bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Animate pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            uniform,

            instances,

            bind_group,
            pipeline,
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue) {
        self.uniform.update(queue);
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        let mut cpass = ctx.encoder.scoped_compute_pass("AnimatePass");

        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &self.bind_group, &[]);
        cpass.set_bind_group(1, &self.uniform.bind_group, &[]);

        const WORKGROUP_SIZE: usize = 256;
        let workgroups_count =
            (self.instances.get().count() as f32 / WORKGROUP_SIZE as f32).ceil() as u32;

        cpass.dispatch_workgroups(workgroups_count, 1, 1);
    }
}
