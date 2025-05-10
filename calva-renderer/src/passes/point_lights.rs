use wgpu::util::DeviceExt;

use crate::{
    util::icosphere::Icosphere, CameraManager, PointLight, PointLightsManager, RenderContext,
    ResourceRef, ResourcesManager,
};

pub struct PointLightsPassInputs<'a> {
    pub albedo_metallic: &'a wgpu::Texture,
    pub normal_roughness: &'a wgpu::Texture,
    pub depth: &'a wgpu::Texture,
    pub output: &'a wgpu::Texture,
}

pub struct PointLightsPass {
    camera: ResourceRef<CameraManager>,
    lights: ResourceRef<PointLightsManager>,

    vertex_count: u32,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,

    output_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,

    stencil_pipeline: wgpu::RenderPipeline,
    lighting_pipeline: wgpu::RenderPipeline,
}

impl PointLightsPass {
    pub fn new(
        device: &wgpu::Device,
        resources: &ResourcesManager,
        inputs: PointLightsPassInputs,
    ) -> Self {
        let camera = resources.get::<CameraManager>();
        let lights = resources.get::<PointLightsManager>();

        let icosphere = Icosphere::new(1);

        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PointLights mesh vertices buffer"),
            contents: bytemuck::cast_slice(&icosphere.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PointLights mesh indices buffer"),
            contents: bytemuck::cast_slice(&icosphere.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let vertex_buffers_layout = [
            // PointLights instances
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<PointLight>() as _,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![
                    0 => Float32x3, // Position
                    1 => Float32,   // Radius
                    2 => Float32x3, // Color
                ],
            },
            // Icosphere vertices
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<[f32; 3]>() as _,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![3 => Float32x3],
            },
        ];

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PointLights shader"),
            source: wgpu::ShaderSource::Wgsl(wesl::include_wesl!("passes::point_lights").into()),
        });

        let stencil_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("PointLights[stencil] pipeline layout"),
                bind_group_layouts: &[&camera.get().bind_group_layout],
                push_constant_ranges: &[],
            });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("PointLights[stencil] pipeline"),
                layout: Some(&pipeline_layout),
                multiview: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main_stencil"),
                    compilation_options: Default::default(),
                    buffers: &vertex_buffers_layout,
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    unclipped_depth: true,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth24PlusStencil8,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState {
                        front: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::Always,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::DecrementWrap,
                            pass_op: wgpu::StencilOperation::Keep,
                        },
                        back: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::Always,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::IncrementWrap,
                            pass_op: wgpu::StencilOperation::Keep,
                        },
                        read_mask: 0x00,
                        write_mask: 0xFF,
                    },
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: Default::default(),
                cache: None,
            })
        };

        let output_view = inputs.output.create_view(&Default::default());
        let depth_view = inputs.depth.create_view(&Default::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("PointLights sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PointLights[lighting] bind group layout"),
            entries: &[
                // sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // albedo + metallic
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // normal + roughness
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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
                    binding: 3,
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

        let bind_group = Self::make_bind_group(device, &bind_group_layout, &sampler, &inputs);

        let lighting_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("PointLights[lighting] pipeline layout"),
                bind_group_layouts: &[&camera.get().bind_group_layout, &bind_group_layout],
                push_constant_ranges: &[],
            });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("PointLights[lighting] pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main_lighting"),
                    compilation_options: Default::default(),
                    buffers: &vertex_buffers_layout,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main_lighting"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: inputs.output.format(),
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: Default::default(),
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: Some(wgpu::Face::Front),
                    unclipped_depth: true,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth24PlusStencil8,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Always,
                    stencil: wgpu::StencilState {
                        front: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::NotEqual,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::Keep,
                            pass_op: wgpu::StencilOperation::Keep,
                        },
                        back: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::NotEqual,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::Keep,
                            pass_op: wgpu::StencilOperation::Keep,
                        },
                        read_mask: 0xFF,
                        write_mask: 0x00,
                    },
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };

        Self {
            camera,
            lights,

            vertex_count: icosphere.count,
            vertices,
            indices,

            output_view,
            depth_view,
            sampler,
            bind_group_layout,
            bind_group,

            stencil_pipeline,
            lighting_pipeline,
        }
    }

    pub fn rebind(&mut self, device: &wgpu::Device, inputs: PointLightsPassInputs) {
        self.bind_group =
            Self::make_bind_group(device, &self.bind_group_layout, &self.sampler, &inputs);

        self.output_view = inputs.output.create_view(&Default::default());
        self.depth_view = inputs.depth.create_view(&Default::default());
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        let mut encoder = ctx.encoder.scope("PointLights");

        let camera = self.camera.get();
        let lights = self.lights.get();

        let mut stencil_pass = encoder.scoped_render_pass(
            "PointLights[stencil]",
            wgpu::RenderPassDescriptor {
                label: Some("PointLights[stencil]"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: None,
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                ..Default::default()
            },
        );

        stencil_pass.set_pipeline(&self.stencil_pipeline);
        stencil_pass.set_bind_group(0, &camera.bind_group, &[]);

        stencil_pass.set_vertex_buffer(0, lights.point_lights.slice(..));
        stencil_pass.set_vertex_buffer(1, self.vertices.slice(..));
        stencil_pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);

        stencil_pass.draw_indexed(0..self.vertex_count, 0, 0..(lights.count() as _));

        drop(stencil_pass);

        let mut lighting_pass = encoder.scoped_render_pass(
            "PointLights[lighting]",
            wgpu::RenderPassDescriptor {
                label: Some("PointLights[lighting]"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: None,
                    stencil_ops: None,
                }),
                ..Default::default()
            },
        );

        lighting_pass.set_pipeline(&self.lighting_pipeline);
        lighting_pass.set_bind_group(0, &camera.bind_group, &[]);
        lighting_pass.set_bind_group(1, &self.bind_group, &[]);

        lighting_pass.set_vertex_buffer(0, lights.point_lights.slice(..));
        lighting_pass.set_vertex_buffer(1, self.vertices.slice(..));
        lighting_pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);

        lighting_pass.draw_indexed(0..self.vertex_count, 0, 0..(lights.count() as _));

        drop(lighting_pass);
    }

    fn make_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        inputs: &PointLightsPassInputs,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PointLights[lighting] bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.albedo_metallic.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.normal_roughness.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&inputs.depth.create_view(
                        &wgpu::TextureViewDescriptor {
                            aspect: wgpu::TextureAspect::DepthOnly,
                            ..Default::default()
                        },
                    )),
                },
            ],
        })
    }
}
