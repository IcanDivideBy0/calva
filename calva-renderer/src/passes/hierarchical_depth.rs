use crate::RenderContext;

pub struct HierarchicalDepthPassInputs<'a> {
    pub depth: &'a wgpu::Texture,
}

pub struct HierarchicalDepthPassOutputs {
    pub output: wgpu::Texture,
}

pub struct HierarchicalDepthPass {
    pub outputs: HierarchicalDepthPassOutputs,
    output_view: wgpu::TextureView,

    size: (u32, u32),
    sampler: wgpu::Sampler,

    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
}

impl HierarchicalDepthPass {
    pub fn new(device: &wgpu::Device, inputs: HierarchicalDepthPassInputs) -> Self {
        let size = (inputs.depth.width() / 16, inputs.depth.height() / 16);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("HierarchicalDepth sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let outputs = Self::make_outputs(device, &inputs);
        let output_view = outputs.output.create_view(&Default::default());

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HierarchicalDepth bind group layout"),
            entries: &[
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Depth input
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Output
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let bind_group =
            Self::make_bind_group(device, &bind_group_layout, &sampler, &output_view, &inputs);

        let shader = device.create_shader_module(wgpu::include_wgsl!("hierarchical_depth.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HierarchicalDepth pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("HierarchicalDepth pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            outputs,
            output_view,

            size,
            sampler,

            bind_group_layout,
            bind_group,
            pipeline,
        }
    }

    pub fn rebind(&mut self, device: &wgpu::Device, inputs: HierarchicalDepthPassInputs) {
        self.size = (inputs.depth.width() / 16, inputs.depth.height() / 16);

        self.outputs = Self::make_outputs(device, &inputs);
        self.output_view = self.outputs.output.create_view(&Default::default());

        self.bind_group = Self::make_bind_group(
            device,
            &self.bind_group_layout,
            &self.sampler,
            &self.output_view,
            &inputs,
        )
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        let mut cpass = ctx
            .encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("HierarchicalDepth"),
                ..Default::default()
            });

        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &self.bind_group, &[]);
        cpass.dispatch_workgroups(self.size.0, self.size.1, 1);
    }

    fn make_outputs(
        device: &wgpu::Device,
        inputs: &HierarchicalDepthPassInputs,
    ) -> HierarchicalDepthPassOutputs {
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HierarchicalDepth output"),
            size: wgpu::Extent3d {
                width: inputs.depth.width() / 16,
                height: inputs.depth.height() / 16,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[wgpu::TextureFormat::R32Float],
        });

        HierarchicalDepthPassOutputs { output }
    }

    fn make_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        output_view: &wgpu::TextureView,
        inputs: &HierarchicalDepthPassInputs,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HierarchicalDepth bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&inputs.depth.create_view(
                        &wgpu::TextureViewDescriptor {
                            aspect: wgpu::TextureAspect::DepthOnly,
                            ..Default::default()
                        },
                    )),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(output_view),
                },
            ],
        })
    }
}
