use anyhow::Result;
use parking_lot::RwLock;
use std::collections::HashMap;
use wesl::syntax::*;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextureHandle(u32);

pub struct TexturesManager {
    mipmaps: MipmapGenerator,

    views: Vec<wgpu::TextureView>,
    sampler: wgpu::Sampler,

    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,
}

impl TexturesManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let mipmaps = MipmapGenerator::new(device);

        let max_textures = device.limits().max_sampled_textures_per_shader_stage;
        let mut views = Vec::with_capacity(max_textures as _);

        views.push(
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("TexturesManager null texture"),
                    size: Default::default(),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[wgpu::TextureFormat::R8Unorm],
                })
                .create_view(&Default::default()),
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("TexturesManager sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TexturesManager bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: core::num::NonZeroU32::new(max_textures),
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = Self::create_bind_group(device, &bind_group_layout, &views, &sampler);

        Self {
            mipmaps,

            views,
            sampler,

            bind_group_layout,
            bind_group,
        }
    }

    pub fn add(&mut self, device: &wgpu::Device, view: wgpu::TextureView) -> TextureHandle {
        self.views.push(view);

        self.bind_group =
            Self::create_bind_group(device, &self.bind_group_layout, &self.views, &self.sampler);

        TextureHandle(self.views.len() as u32 - 1)
    }

    pub fn generate_mipmaps(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        desc: &wgpu::TextureDescriptor,
    ) -> Result<()> {
        self.mipmaps.generate_mipmaps(device, queue, texture, desc)
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        views: &[wgpu::TextureView],
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TexturesManager bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureViewArray(
                        &views.iter().collect::<Vec<_>>(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}

impl From<&wgpu::Device> for TexturesManager {
    fn from(device: &wgpu::Device) -> Self {
        Self::new(device)
    }
}

struct MipmapGenerator {
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,

    shader: wgpu::ShaderModule,
    pipeline_layout: wgpu::PipelineLayout,
    pipelines: RwLock<HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>>,
}

impl MipmapGenerator {
    fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("MipmapGenerator shader"),
            source: wgpu::ShaderSource::Wgsl(
                wesl::quote_module! {
                    struct VertexOutput {
                        @builtin(position) position: vec4<f32>,
                        @location(0) uv: vec2<f32>,
                    };

                    @vertex
                    fn vs_main(@builtin(vertex_index) vertex_index : u32) -> VertexOutput {
                        let tc = vec2<f32>(
                            f32(vertex_index >> 1u),
                            f32(vertex_index &  1u),
                        ) * 2.0;

                        return VertexOutput(
                            vec4<f32>(tc * 2.0 - 1.0, 0.0, 1.0),
                            vec2<f32>(tc.x, 1.0 - tc.y)
                        );
                    }

                    @group(0) @binding(0) var t_input: texture_2d<f32>;
                    @group(0) @binding(1) var t_sampler: sampler;

                    @fragment
                    fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                        return textureSample(t_input, t_sampler, in.uv);
                    }
                }
                .to_string()
                .into(),
            ),
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("MipmapGenerator sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MipmapGenerator bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
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
            label: Some("MipmapGenerator render pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        Self {
            sampler,
            bind_group_layout,

            shader,
            pipeline_layout,
            pipelines: Default::default(),
        }
    }

    fn create_pipeline(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("MipmapGenerator render pipeline"),
            layout: Some(&self.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        })
    }

    pub fn generate_mipmaps(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        desc: &wgpu::TextureDescriptor,
    ) -> Result<()> {
        let pipelines_read = self.pipelines.read();

        let pipeline = match pipelines_read.get(&desc.format) {
            Some(pipeline) => pipeline,
            None => {
                drop(pipelines_read);

                self.pipelines
                    .write()
                    .insert(desc.format, self.create_pipeline(device, desc.format));

                return self.generate_mipmaps(device, queue, texture, desc);
            }
        };

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("MipmapGenerator command encoder"),
        });

        let mips = (0..desc.size.max_mips(desc.dimension))
            .map(|mip_level| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: mip_level,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect::<Vec<_>>();

        for res in mips.windows(2).map(<&[_; 2]>::try_from) {
            let [input, output] = res?;

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("MipmapGenerator bind group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(input),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });

            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("MipmapGenerator render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            rpass.set_pipeline(pipeline);
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.draw(0..3, 0..1);
        }

        queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }
}
