use calva::renderer::{
    wgpu, MeshInstance, MeshInstances, RenderContext, SkinAnimationInstance,
    SkinAnimationInstances, SkinAnimations,
};

pub struct Particles {
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
}

impl Particles {
    pub fn new(
        device: &wgpu::Device,
        mesh_instances: &MeshInstances,
        animation_instances: &SkinAnimationInstances,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particles shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("particle.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Particles bind group layout"),
            entries: &[
                // Mesh instances
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<MeshInstance>() as _,
                        ),
                    },
                    count: None,
                },
                // Animation instances
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<
                            SkinAnimationInstance,
                        >() as _),
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particles bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: mesh_instances.buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: animation_instances.buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Particles pipeline layout"),
            bind_group_layouts: &[
                &bind_group_layout,
                &device.create_bind_group_layout(SkinAnimations::DESC),
            ],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Particles pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });

        Self {
            bind_group,
            pipeline,
        }
    }

    pub fn run(&self, ctx: &mut RenderContext, animations: &SkinAnimations) {
        let mut cpass = ctx
            .encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Particles compute pass"),
            });

        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &self.bind_group, &[]);
        cpass.set_bind_group(1, &animations.bind_group, &[]);
        cpass.dispatch_workgroups(100, 1, 1);
    }
}
