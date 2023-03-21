use wgpu::util::DeviceExt;

use crate::{CameraManager, RenderContext};

pub struct Skybox {
    bind_group: wgpu::BindGroup,
}

pub struct SkyboxPassInputs<'a> {
    pub depth: &'a wgpu::Texture,
    pub output: &'a wgpu::Texture,
}

pub struct SkyboxPass {
    depth_view: wgpu::TextureView,
    output_view: wgpu::TextureView,

    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

impl SkyboxPass {
    pub fn new(device: &wgpu::Device, camera: &CameraManager, inputs: SkyboxPassInputs) -> Self {
        let output_view = inputs.output.create_view(&Default::default());
        let depth_view = inputs.depth.create_view(&Default::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Skybox sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Skybox bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Skybox render pipeline layout"),
            bind_group_layouts: &[&camera.bind_group_layout, &bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("skybox.wgsl"));

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skybox render pipeline"),
            layout: Some(&pipeline_layout),
            multiview: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: inputs.output.format(),
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
        });

        Self {
            output_view,
            depth_view,

            sampler,
            bind_group_layout,
            pipeline,
        }
    }

    pub fn rebind(&mut self, inputs: SkyboxPassInputs) {
        self.output_view = inputs.output.create_view(&Default::default());
        self.depth_view = inputs.depth.create_view(&Default::default());
    }

    pub fn render(&self, ctx: &mut RenderContext, camera: &CameraManager, skybox: &Skybox) {
        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Skybox"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: None,
                stencil_ops: None,
            }),
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &camera.bind_group, &[]);
        rpass.set_bind_group(1, &skybox.bind_group, &[]);

        rpass.draw(0..3, 0..1);
    }

    pub fn create_skybox(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pixels: &[u8],
    ) -> Skybox {
        let size = (pixels.len() as f32 / (4.0 * 6.0)).sqrt() as _;

        let view = device
            .create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("Skybox texture"),
                    size: wgpu::Extent3d {
                        width: size,
                        height: size,
                        depth_or_array_layers: 6,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
                },
                pixels,
            )
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Skybox texture view"),
                dimension: Some(wgpu::TextureViewDimension::Cube),
                array_layer_count: std::num::NonZeroU32::new(6),
                ..Default::default()
            });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skybox bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        Skybox { bind_group }
    }
}
