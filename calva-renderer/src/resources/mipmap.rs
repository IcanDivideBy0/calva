use anyhow::Result;
use parking_lot::RwLock;
use std::collections::HashMap;
use wesl::syntax::*;

use crate::{Resource, ResourcesManager};

pub struct MipmapGenerator {
    resources: ResourcesManager,

    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,

    shader: wgpu::ShaderModule,
    pipeline_layout: wgpu::PipelineLayout,
    pipelines: RwLock<HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>>,
}

impl MipmapGenerator {
    fn new(resources: &ResourcesManager) -> Self {
        let resources = resources.clone();
        let device = resources.read::<wgpu::Device>();

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("MipmapGenerator shader"),
            source: wgpu::ShaderSource::Wgsl(
                wesl_quote::quote_module! {
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
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        Self {
            resources,

            sampler,
            bind_group_layout,

            shader,
            pipeline_layout,
            pipelines: Default::default(),
        }
    }

    fn create_pipeline(&self, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let device = self.resources.read::<wgpu::Device>();

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
            multiview_mask: None,
            cache: None,
        })
    }

    pub fn generate_mipmaps(
        &self,
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
                    .insert(desc.format, self.create_pipeline(desc.format));

                return self.generate_mipmaps(texture, desc);
            }
        };

        let device = self.resources.read::<wgpu::Device>();
        let queue = self.resources.read::<wgpu::Queue>();

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

impl Resource for MipmapGenerator {
    fn instanciate(resources: &ResourcesManager) -> Self {
        Self::new(resources)
    }
}
