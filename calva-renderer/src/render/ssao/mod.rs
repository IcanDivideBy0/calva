use wgpu::util::DeviceExt;

use crate::{CameraManager, GeometryPass, RenderContext, Renderer};

mod blit;
mod blur;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SsaoConfig {
    pub radius: f32,
    pub bias: f32,
    pub power: f32,
}

impl SsaoConfig {
    pub const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as wgpu::BufferAddress;
}

impl Default for SsaoConfig {
    fn default() -> Self {
        Self {
            radius: 0.3,
            bias: 0.025,
            power: 1.0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SsaoRandomUniform {
    samples: [glam::Vec4; SsaoRandomUniform::SAMPLES_COUNT],
    noise: [glam::Vec4; 16],
}

impl SsaoRandomUniform {
    pub const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as wgpu::BufferAddress;

    const SAMPLES_COUNT: usize = 32;

    fn new() -> Self {
        let samples = (0..Self::SAMPLES_COUNT)
            .map(|i| {
                let sample = glam::vec4(
                    rand::random::<f32>() * 2.0 - 1.0,
                    rand::random::<f32>() * 2.0 - 1.0,
                    rand::random::<f32>(),
                    0.0,
                )
                .normalize();

                let scale = i as f32 / Self::SAMPLES_COUNT as f32;
                sample
                    * glam::Vec4::lerp(
                        glam::Vec4::splat(0.1),
                        glam::Vec4::splat(1.0),
                        scale * scale,
                    )
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let noise = (0..16)
            .map(|_| {
                glam::vec4(
                    rand::random::<f32>() * 2.0 - 1.0,
                    rand::random::<f32>() * 2.0 - 1.0,
                    0.0,
                    0.0,
                )
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        Self { samples, noise }
    }
}

pub struct SsaoPass<const WIDTH: u32, const HEIGHT: u32> {
    config_buffer: wgpu::Buffer,
    random_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,

    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,

    output: wgpu::TextureView,
    blur: blur::SsaoBlur<WIDTH, HEIGHT>,
    blit: blit::SsaoBlit,
}

impl<const WIDTH: u32, const HEIGHT: u32> SsaoPass<WIDTH, HEIGHT> {
    const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

    pub fn new(renderer: &Renderer, camera: &CameraManager, geometry: &GeometryPass) -> Self {
        let config_buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Ssao config buffer"),
            size: SsaoConfig::SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let random_buffer = renderer
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Ssao uniforms buffer"),
                contents: bytemuck::bytes_of(&SsaoRandomUniform::new()),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let sampler = renderer.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Ssao sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let bind_group_layout =
            renderer
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Ssao bind group layout"),
                    entries: &[
                        // config
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(SsaoConfig::SIZE),
                            },
                            count: None,
                        },
                        // random data
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(SsaoRandomUniform::SIZE),
                            },
                            count: None,
                        },
                        // sampler
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        // normals
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        // depth
                        wgpu::BindGroupLayoutEntry {
                            binding: 4,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Depth,
                            },
                            count: None,
                        },
                    ],
                });

        let bind_group = Self::make_bind_group(
            renderer,
            geometry,
            &bind_group_layout,
            &config_buffer,
            &random_buffer,
            &sampler,
        );

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Ssao pipeline layout"),
                    bind_group_layouts: &[&camera.bind_group_layout, &bind_group_layout],
                    push_constant_ranges: &[],
                });

        let shader = renderer
            .device
            .create_shader_module(wgpu::include_wgsl!("ssao.wgsl"));

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Ssao pipeline"),
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
                multiview: None,
                multisample: Default::default(),
            });

        let output = Self::make_texture(renderer, Some("Ssao output"));

        let blur = blur::SsaoBlur::new(renderer, &output);
        let blit = blit::SsaoBlit::new(renderer, &output);

        Self {
            config_buffer,
            random_buffer,
            sampler,

            bind_group_layout,
            bind_group,
            pipeline,

            output,
            blur,
            blit,
        }
    }

    pub fn rebind(&mut self, renderer: &Renderer, geometry: &GeometryPass) {
        self.bind_group = Self::make_bind_group(
            renderer,
            geometry,
            &self.bind_group_layout,
            &self.config_buffer,
            &self.random_buffer,
            &self.sampler,
        );
    }

    pub fn update(&self, renderer: &Renderer, config: &SsaoConfig) {
        renderer
            .queue
            .write_buffer(&self.config_buffer, 0, bytemuck::bytes_of(config));
    }

    pub fn render(
        &self,
        ctx: &mut RenderContext,
        output: &wgpu::TextureView,
        camera: &CameraManager,
    ) {
        ctx.encoder.profile_start("Ssao");

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Ssao[render]"),
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
        rpass.set_bind_group(0, &camera.bind_group, &[]);
        rpass.set_bind_group(1, &self.bind_group, &[]);

        rpass.draw(0..3, 0..1);

        drop(rpass);

        self.blur.render(ctx, &self.output);
        self.blit.render(ctx, output);

        ctx.encoder.profile_end();
    }

    fn make_texture(renderer: &Renderer, label: wgpu::Label<'static>) -> wgpu::TextureView {
        renderer
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label,
                size: wgpu::Extent3d {
                    width: WIDTH,
                    height: HEIGHT,
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
        config_buffer: &wgpu::Buffer,
        random_buffer: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Ssao bind group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: config_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: random_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&geometry.normal_roughness),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&renderer.depth),
                    },
                ],
            })
    }
}
