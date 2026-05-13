use anyhow::Result;

use crate::{GeometryPassOutputs, RenderContext, Resource, ResourcesManager};

pub struct HierarchicalDepthPassInputs<'a> {
    pub depth: &'a wgpu::Texture,
}

pub struct HierarchicalDepthPassOutputs {
    pub output: wgpu::Texture,
    pub output_view: wgpu::TextureView,
}

impl Resource for HierarchicalDepthPassOutputs {
    fn instanciate(resources: &ResourcesManager) -> Result<Self> {
        let device = resources.read::<wgpu::Device>();
        let surface_config = resources.read::<wgpu::SurfaceConfiguration>();

        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HierarchicalDepth output"),
            size: wgpu::Extent3d {
                width: surface_config.width / 16,
                height: surface_config.height / 16,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[wgpu::TextureFormat::R32Float],
        });

        let output_view = output.create_view(&Default::default());

        Ok(Self {
            output,
            output_view,
        })
    }

    fn update(&mut self, resources: &ResourcesManager) -> Result<()> {
        let surface_config = resources.read::<wgpu::SurfaceConfiguration>();

        let size = wgpu::Extent3d {
            width: surface_config.width / 16,
            height: surface_config.height / 16,
            depth_or_array_layers: 1,
        };

        if self.output.size() != size {
            *self = Self::instanciate(resources)?;
        }

        Ok(())
    }
}

pub struct HierarchicalDepthPass {
    resources: ResourcesManager,

    sampler: wgpu::Sampler,

    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
}

impl HierarchicalDepthPass {
    pub fn new(resources: &ResourcesManager) -> Self {
        let resources = resources.clone();
        let device = resources.read::<wgpu::Device>();
        let geometry_outputs = resources.read::<GeometryPassOutputs>();
        let hierarchical_depth_outputs = resources.read::<HierarchicalDepthPassOutputs>();

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("HierarchicalDepth sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

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

        let bind_group = Self::make_bind_group(
            &device,
            &bind_group_layout,
            &sampler,
            &hierarchical_depth_outputs.output_view,
            &geometry_outputs.depth,
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HierarchicalDepth shader"),
            source: wgpu::ShaderSource::Wgsl(
                wesl::include_wesl!("passes::hierarchical_depth").into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HierarchicalDepth pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
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
            resources,

            sampler,

            bind_group_layout,
            bind_group,
            pipeline,
        }
    }

    pub fn rebind(&mut self) {
        let device = self.resources.read::<wgpu::Device>();
        let geometry_outputs = self.resources.read::<GeometryPassOutputs>();
        let hierarchical_depth_outputs = self.resources.read::<HierarchicalDepthPassOutputs>();

        self.bind_group = Self::make_bind_group(
            &device,
            &self.bind_group_layout,
            &self.sampler,
            &hierarchical_depth_outputs.output_view,
            &geometry_outputs.depth,
        )
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        let hierarchical_depth_outputs = self.resources.read::<HierarchicalDepthPassOutputs>();

        let mut cpass = ctx.encoder.scoped_compute_pass("HierarchicalDepth");

        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &self.bind_group, &[]);
        cpass.dispatch_workgroups(
            hierarchical_depth_outputs.output.width(),
            hierarchical_depth_outputs.output.height(),
            1,
        );
    }

    fn make_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        output_view: &wgpu::TextureView,
        depth: &wgpu::Texture,
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
                    resource: wgpu::BindingResource::TextureView(&depth.create_view(
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
