use crate::{GeometryPass, RenderContext, Renderer};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AmbientLightConfig {
    pub factor: f32,
}

impl Default for AmbientLightConfig {
    fn default() -> Self {
        Self { factor: 0.005 }
    }
}

pub struct AmbientLightPass {
    pub output: wgpu::TextureView,

    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl AmbientLightPass {
    pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    pub fn new(renderer: &Renderer, geometry: &GeometryPass) -> Self {
        let output = Self::make_texture(renderer);

        let bind_group_layout =
            renderer
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("AmbientLight bind group layout"),
                    entries: &[
                        // albedo
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            },
                            count: None,
                        },
                        // emissive
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            },
                            count: None,
                        },
                    ],
                });

        let bind_group = Self::make_bind_group(renderer, geometry, &bind_group_layout);

        let shader = renderer
            .device
            .create_shader_module(wgpu::include_wgsl!("ambient_light.wgsl"));

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("AmbientLight pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[wgpu::PushConstantRange {
                        stages: wgpu::ShaderStages::FRAGMENT,
                        range: 0..(std::mem::size_of::<f32>() as _),
                    }],
                });

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("AmbientLight pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: Self::OUTPUT_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
            });

        Self {
            output,

            bind_group_layout,
            bind_group,
            pipeline,
        }
    }

    pub fn rebind(&mut self, renderer: &Renderer, geometry: &GeometryPass) {
        self.output = Self::make_texture(renderer);
        self.bind_group = Self::make_bind_group(renderer, geometry, &self.bind_group_layout);
    }

    pub fn render(&self, ctx: &mut RenderContext, config: &AmbientLightConfig) {
        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("AmbientLight"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.output,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_push_constants(wgpu::ShaderStages::FRAGMENT, 0, bytemuck::bytes_of(config));

        rpass.draw(0..3, 0..1);
    }

    fn make_texture(renderer: &Renderer) -> wgpu::TextureView {
        renderer
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("AmbientLight output"),
                size: wgpu::Extent3d {
                    width: renderer.surface_config.width,
                    height: renderer.surface_config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::OUTPUT_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[Self::OUTPUT_FORMAT],
            })
            .create_view(&Default::default())
    }

    fn make_bind_group(
        renderer: &Renderer,
        geometry: &GeometryPass,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("AmbientLight bind group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&geometry.albedo_metallic),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&geometry.emissive),
                    },
                ],
            })
    }
}
