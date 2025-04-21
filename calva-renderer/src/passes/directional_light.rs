use crate::{
    AnimationState, AnimationsManager, Camera, CameraManager, DirectionalLight, MaterialId,
    MeshesManager, RenderContext, RessourceRef, RessourcesManager, SkinsManager, UniformBuffer,
    UniformData,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawInstance {
    _model_matrix: [f32; 16],
    _material: MaterialId,
    _skin_offset: i32,
    _animation: AnimationState,
}

impl DrawInstance {
    pub(crate) const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;

    pub(crate) const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: Self::SIZE,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            // Model matrix
            0 => Float32x4,
            1 => Float32x4,
            2 => Float32x4,
            3 => Float32x4,

            4 => Uint32, // Material

            // Skinning
            5 => Sint32, // Skin offset
            6 => Uint32, // Animation ID
            7 => Float32, // Animation time
        ],
    };
}

pub struct DirectionalLightPassInputs<'a> {
    pub albedo_metallic: &'a wgpu::Texture,
    pub normal_roughness: &'a wgpu::Texture,
    pub depth: &'a wgpu::Texture,
    pub output: &'a wgpu::Texture,
}

pub struct DirectionalLightPass {
    pub uniform: UniformBuffer<DirectionalLightUniform>,

    camera: RessourceRef<CameraManager>,
    meshes: RessourceRef<MeshesManager>,
    skins: RessourceRef<SkinsManager>,
    animations: RessourceRef<AnimationsManager>,

    output_view: wgpu::TextureView,
    cull: DirectionalLightCull,

    sampler: wgpu::Sampler,

    light_depth_view: wgpu::TextureView,
    light_depth_pipeline: wgpu::RenderPipeline,

    blur_pass: DirectionalLightBlur,

    lighting_bind_group_layout: wgpu::BindGroupLayout,
    lighting_bind_group: wgpu::BindGroup,
    lighting_pipeline: wgpu::RenderPipeline,
}

impl DirectionalLightPass {
    const SIZE: u32 = 2048;
    const TEXTURE_SIZE: wgpu::Extent3d = wgpu::Extent3d {
        width: Self::SIZE,
        height: Self::SIZE,
        depth_or_array_layers: 1,
    };

    pub fn new(
        device: &wgpu::Device,
        ressources: &RessourcesManager,
        inputs: DirectionalLightPassInputs,
    ) -> Self {
        let uniform = UniformBuffer::new(device, DirectionalLightUniform::default());

        let camera = ressources.get::<CameraManager>();
        let meshes = ressources.get::<MeshesManager>();
        let skins = ressources.get::<SkinsManager>();
        let animations = ressources.get::<AnimationsManager>();

        let cull = DirectionalLightCull::new(device, ressources, &uniform);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("DirectionalLight sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let output_view = inputs.output.create_view(&Default::default());

        let light_depth = Self::make_depth_texture(device, Some("DirectionalLight depth texture"));
        let light_depth_view = light_depth.create_view(&Default::default());

        let light_depth_pipeline = {
            let shader =
                device.create_shader_module(wgpu::include_wgsl!("directional_light.depth.wgsl",));

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("DirectionalLight[depth] render pipeline layout"),
                bind_group_layouts: &[
                    &uniform.bind_group_layout,
                    &skins.get().bind_group_layout,
                    &animations.get().bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("DirectionalLight[depth] render pipeline"),
                layout: Some(&pipeline_layout),
                multiview: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[
                        DrawInstance::LAYOUT,
                        // Positions
                        wgpu::VertexBufferLayout {
                            array_stride: MeshesManager::VERTEX_SIZE as _,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![10 => Float32x3],
                        },
                    ],
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    unclipped_depth: true,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: light_depth.format(),
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                cache: None,
            })
        };

        let blur_pass = blur::DirectionalLightBlur::new(device, &light_depth);

        let (lighting_bind_group_layout, lighting_bind_group, lighting_pipeline) = {
            let shader = device
                .create_shader_module(wgpu::include_wgsl!("directional_light.lighting.wgsl",));

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("DirectionalLight[lighting] bind group layout"),
                    entries: &[
                        // albedo + metallic
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
                        // normal + roughness
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
                        // depth
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Depth,
                            },
                            count: None,
                        },
                        // shadows
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
                        // shadows sampler
                        wgpu::BindGroupLayoutEntry {
                            binding: 4,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

            let bind_group = Self::make_lighting_bind_group(
                device,
                &bind_group_layout,
                &light_depth_view,
                &sampler,
                &inputs,
            );

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("DirectionalLight[lighting] pipeline layout"),
                bind_group_layouts: &[
                    &camera.get().bind_group_layout,
                    &uniform.bind_group_layout,
                    &bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("DirectionalLight[lighting] pipeline"),
                layout: Some(&pipeline_layout),
                multiview: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
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
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                cache: None,
            });

            (bind_group_layout, bind_group, pipeline)
        };

        Self {
            uniform,

            camera,
            meshes,
            skins,
            animations,

            cull,

            output_view,
            sampler,
            light_depth_view,
            light_depth_pipeline,

            blur_pass,

            lighting_bind_group_layout,
            lighting_bind_group,
            lighting_pipeline,
        }
    }

    pub fn rebind(&mut self, device: &wgpu::Device, inputs: DirectionalLightPassInputs) {
        self.lighting_bind_group = Self::make_lighting_bind_group(
            device,
            &self.lighting_bind_group_layout,
            &self.light_depth_view,
            &self.sampler,
            &inputs,
        );

        self.output_view = inputs.output.create_view(&Default::default());
    }

    pub fn update(&mut self, queue: &wgpu::Queue) {
        self.uniform.camera = ***self.camera.get();
        self.uniform.update(queue);
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        let mut encoder = ctx.encoder.scope("DirectionalLight");

        let camera = self.camera.get();
        let meshes = self.meshes.get();
        let skins = self.skins.get();
        let animations = self.animations.get();

        self.cull.cull(&mut encoder, &self.uniform);

        let mut depth_pass = encoder.scoped_render_pass(
            "DirectionalLight[depth]",
            wgpu::RenderPassDescriptor {
                label: Some("DirectionalLight[depth]"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.light_depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            },
        );

        depth_pass.set_pipeline(&self.light_depth_pipeline);

        depth_pass.set_bind_group(0, &self.uniform.bind_group, &[]);
        depth_pass.set_bind_group(1, &skins.bind_group, &[]);
        depth_pass.set_bind_group(2, &animations.bind_group, &[]);

        depth_pass.set_vertex_buffer(0, self.cull.draw_instances.slice(..));
        depth_pass.set_vertex_buffer(1, meshes.vertices.slice(..));

        depth_pass.set_index_buffer(meshes.indices.slice(..), wgpu::IndexFormat::Uint32);

        depth_pass.multi_draw_indexed_indirect_count(
            &self.cull.draw_indirects,
            std::mem::size_of::<u32>() as _,
            &self.cull.draw_indirects,
            0,
            MeshesManager::MAX_MESHES as _,
        );

        drop(depth_pass);

        self.blur_pass.render(&mut encoder);

        let mut lighting_pass = encoder.scoped_render_pass(
            "DirectionalLight[lighting]",
            wgpu::RenderPassDescriptor {
                label: Some("DirectionalLight[lighting]"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            },
        );

        lighting_pass.set_pipeline(&self.lighting_pipeline);

        lighting_pass.set_bind_group(0, &camera.bind_group, &[]);
        lighting_pass.set_bind_group(1, &self.uniform.bind_group, &[]);
        lighting_pass.set_bind_group(2, &self.lighting_bind_group, &[]);

        lighting_pass.draw(0..3, 0..1);

        drop(lighting_pass);
    }

    fn make_lighting_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        light_depth: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        inputs: &DirectionalLightPassInputs,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DirectionalLight[lighting] bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.albedo_metallic.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.normal_roughness.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&inputs.depth.create_view(
                        &wgpu::TextureViewDescriptor {
                            aspect: wgpu::TextureAspect::DepthOnly,
                            ..Default::default()
                        },
                    )),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(light_depth),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    fn make_depth_texture(device: &wgpu::Device, label: wgpu::Label<'static>) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: Self::TEXTURE_SIZE,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth16Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[wgpu::TextureFormat::Depth16Unorm],
        })
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuDirectionalLightUniform {
    color: glam::Vec4,
    direction_world: glam::Vec4,
    direction_view: glam::Vec4,
    view_proj: glam::Mat4,
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct DirectionalLightUniform {
    pub light: DirectionalLight,
    camera: Camera,
}

impl UniformData for DirectionalLightUniform {
    type GpuType = GpuDirectionalLightUniform;

    fn as_gpu_type(&self) -> Self::GpuType {
        let light_dir = self.light.direction.normalize();
        let light_view = glam::Mat4::look_at_rh(glam::Vec3::ZERO, light_dir, glam::Vec3::Y);

        // Frustum bounding sphere in view space
        // https://lxjk.github.io/2017/04/15/Calculate-Minimal-Bounding-Sphere-of-Frustum.html
        // https://stackoverflow.com/questions/2194812/finding-a-minimum-bounding-sphere-for-a-frustum
        // https://stackoverflow.com/questions/56428880/how-to-extract-camera-parameters-from-projection-matrix
        let proj = self.camera.proj;
        let znear = proj.w_axis.z / (proj.z_axis.z - 1.0);
        let zfar = proj.w_axis.z / (proj.z_axis.z + 1.0);

        let k = f32::sqrt(1.0 + (proj.x_axis.x / proj.y_axis.y).powi(2)) * proj.x_axis.x.recip();
        let k2 = k.powi(2);

        let (mut center, mut radius) = if k2 >= (zfar - znear) / (zfar + znear) {
            (glam::vec3(0.0, 0.0, -zfar), zfar * k)
        } else {
            (
                glam::vec3(0.0, 0.0, -0.5 * (zfar + znear) * (1.0 + k2)),
                0.5 * f32::sqrt(
                    f32::powi(zfar - znear, 2)
                        + 2.0 * (zfar.powi(2) + znear.powi(2)) * k2
                        + f32::powi(zfar + znear, 2) * k.powi(4),
                ),
            )
        };

        // Move sphere to light view space
        center = (light_view * self.camera.view.inverse() * center.extend(1.0)).truncate();

        // Avoid shadow swimming:
        // 1. prevent small radius changes due to float precision
        radius = (radius * 16.0).ceil() / 16.0;
        // 2. shadow texel size in light view space
        let texel_size = radius * 2.0 / DirectionalLightPass::SIZE as f32;
        // 3. allow center changes only in texel size increments
        center = (center / texel_size).ceil() * texel_size;

        let min = center - glam::Vec3::splat(radius);
        let max = center + glam::Vec3::splat(radius);

        let light_proj = glam::Mat4::orthographic_rh(
            min.x,  // left
            max.x,  // right
            min.y,  // bottom
            max.y,  // top
            -max.z, // near
            -min.z, // far
        );

        GpuDirectionalLightUniform {
            color: (glam::Vec3::from_array(self.light.color) * self.light.intensity).extend(1.0),
            direction_world: light_dir.extend(0.0),
            direction_view: (glam::Quat::from_mat4(&self.camera.view) * light_dir).extend(0.0),
            view_proj: (light_proj * light_view),
        }
    }
}

use cull::*;
mod cull {
    use crate::{
        CameraManager, Instance, InstancesManager, MeshInfo, MeshesManager, ProfilerCommandEncoder,
        RessourceRef, RessourcesManager, UniformBuffer,
    };

    use super::{DirectionalLightUniform, DrawInstance};

    pub struct DirectionalLightCull {
        camera: RessourceRef<CameraManager>,
        meshes: RessourceRef<MeshesManager>,
        instances: RessourceRef<InstancesManager>,

        pub(crate) draw_instances: wgpu::Buffer,
        pub(crate) draw_indirects: wgpu::Buffer,

        bind_group: wgpu::BindGroup,
        pipelines: (
            wgpu::ComputePipeline, // reset
            wgpu::ComputePipeline, // cull
            wgpu::ComputePipeline, // count
        ),
    }

    impl DirectionalLightCull {
        pub fn new(
            device: &wgpu::Device,
            ressources: &RessourcesManager,
            uniform: &UniformBuffer<DirectionalLightUniform>,
        ) -> Self {
            let camera = ressources.get::<CameraManager>();
            let meshes = ressources.get::<MeshesManager>();
            let instances = ressources.get::<InstancesManager>();

            let draw_instances = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("DirectionalLight[cull] draw instances"),
                size: (std::mem::size_of::<[DrawInstance; InstancesManager::MAX_INSTANCES]>()) as _,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            });

            let draw_indirects = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("DirectionalLight[cull] draw indirects"),
                size: {
                    let count_size = std::mem::size_of::<u32>();
                    let indirects_size = std::mem::size_of::<
                        [wgpu::util::DrawIndexedIndirectArgs; MeshesManager::MAX_MESHES],
                    >();

                    count_size + indirects_size
                } as _,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::INDIRECT,
                mapped_at_creation: false,
            });

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("DirectionalLight[cull] bind group layout"),
                    entries: &[
                        // Mesh data
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(MeshInfo::SIZE),
                            },
                            count: None,
                        },
                        // Base instances
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(
                                    std::mem::size_of::<u32>() as _
                                ),
                            },
                            count: None,
                        },
                        // Instances
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(
                                    std::mem::size_of::<[u32; 4]>() as wgpu::BufferAddress
                                        + Instance::SIZE,
                                ),
                            },
                            count: None,
                        },
                        // Draw instances
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(DrawInstance::SIZE),
                            },
                            count: None,
                        },
                        // Draw indirects
                        wgpu::BindGroupLayoutEntry {
                            binding: 4,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(
                                    std::mem::size_of::<u32>() as u64
                                        + std::mem::size_of::<wgpu::util::DrawIndexedIndirectArgs>()
                                            as u64,
                                ),
                            },
                            count: None,
                        },
                    ],
                });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("DirectionalLight[cull] bind group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: meshes.get().meshes_info.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: instances.get().base_instances.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: instances.get().instances.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: draw_instances.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: draw_indirects.as_entire_binding(),
                    },
                ],
            });

            let shader =
                device.create_shader_module(wgpu::include_wgsl!("directional_light.cull.wgsl"));

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("DirectionalLight[cull] pipeline layout"),
                bind_group_layouts: &[
                    &camera.get().bind_group_layout,
                    &uniform.bind_group_layout,
                    &bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

            let pipelines = (
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("DirectionalLight[cull] reset pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some("reset"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("DirectionalLight[cull] cull pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some("cull"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("DirectionalLight[cull] count pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some("count"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
            );

            Self {
                camera,
                meshes,
                instances,

                draw_instances,
                draw_indirects,

                bind_group,
                pipelines,
            }
        }

        pub fn cull(
            &self,
            encoder: &mut ProfilerCommandEncoder,
            uniform: &UniformBuffer<DirectionalLightUniform>,
        ) {
            let camera = self.camera.get();

            let mut cpass = encoder.scoped_compute_pass("DirectionalLight[cull]");

            const WORKGROUP_SIZE: u32 = 32;

            let meshes_count: u32 = self.meshes.get().count();
            let meshes_workgroups_count =
                (meshes_count as f32 / WORKGROUP_SIZE as f32).ceil() as u32;

            let instances_count: u32 = self.instances.get().count();
            let instances_workgroups_count =
                (instances_count as f32 / WORKGROUP_SIZE as f32).ceil() as u32;

            cpass.set_pipeline(&self.pipelines.0);
            cpass.set_bind_group(0, &camera.bind_group, &[]);
            cpass.set_bind_group(1, &uniform.bind_group, &[]);
            cpass.set_bind_group(2, &self.bind_group, &[]);
            cpass.dispatch_workgroups(meshes_workgroups_count, 1, 1);

            cpass.set_pipeline(&self.pipelines.1);
            cpass.set_bind_group(0, &camera.bind_group, &[]);
            cpass.set_bind_group(1, &uniform.bind_group, &[]);
            cpass.set_bind_group(2, &self.bind_group, &[]);
            cpass.dispatch_workgroups(instances_workgroups_count, 1, 1);

            cpass.set_pipeline(&self.pipelines.2);
            cpass.set_bind_group(0, &camera.bind_group, &[]);
            cpass.set_bind_group(1, &uniform.bind_group, &[]);
            cpass.set_bind_group(2, &self.bind_group, &[]);
            cpass.dispatch_workgroups(meshes_workgroups_count, 1, 1);
        }
    }
}

use blur::*;
mod blur {
    use crate::ProfilerCommandEncoder;

    use super::DirectionalLightPass;

    #[derive(Clone, Copy)]
    enum Direction {
        Horizontal,
        Vertical,
    }

    impl std::fmt::Display for Direction {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(match self {
                Direction::Horizontal => "horizontal",
                Direction::Vertical => "vertical",
            })
        }
    }

    pub struct DirectionalLightBlur {
        temp_view: wgpu::TextureView,
        output_view: wgpu::TextureView,

        h_pass: wgpu::RenderBundle,
        v_pass: wgpu::RenderBundle,
    }

    impl DirectionalLightBlur {
        pub fn new(device: &wgpu::Device, output: &wgpu::Texture) -> Self {
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("DirectionalLightBlur sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });

            let temp = DirectionalLightPass::make_depth_texture(
                device,
                Some("DirectionalLightBlur temp texture"),
            );
            let temp_view = temp.create_view(&Default::default());
            let output_view = output.create_view(&Default::default());

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("DirectionalLightBlur bind group layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
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

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("DirectionalLightBlur pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let shader =
                device.create_shader_module(wgpu::include_wgsl!("directional_light.blur.wgsl"));

            let make_render_bundle = |direction: Direction| {
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(format!("DirectionalLightBlur[{direction}] bind group").as_str()),
                    layout: &bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(match direction {
                                Direction::Horizontal => &output_view,
                                Direction::Vertical => &temp_view,
                            }),
                        },
                    ],
                });

                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(format!("DirectionalLightBlur[{direction}] pipeline").as_str()),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some(format!("fs_main_{direction}").as_str()),
                        compilation_options: Default::default(),
                        targets: &[],
                    }),
                    primitive: Default::default(),
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: output.format(),
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Always,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: Default::default(),
                    multiview: None,
                    cache: None,
                });

                let mut encoder =
                    device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                        label: Some(
                            format!("DirectionalLightBlur[{direction}] render bundle").as_str(),
                        ),
                        color_formats: &[],
                        depth_stencil: Some(wgpu::RenderBundleDepthStencil {
                            format: output.format(),
                            depth_read_only: false,
                            stencil_read_only: false,
                        }),
                        sample_count: 1,
                        multiview: None,
                    });

                encoder.set_pipeline(&pipeline);
                encoder.set_bind_group(0, &bind_group, &[]);

                encoder.draw(0..3, 0..1);

                encoder.finish(&Default::default())
            };

            let h_pass = make_render_bundle(Direction::Horizontal);
            let v_pass = make_render_bundle(Direction::Vertical);

            Self {
                temp_view,
                output_view,

                h_pass,
                v_pass,
            }
        }

        pub fn render(&self, encoder: &mut ProfilerCommandEncoder) {
            let mut encoder = encoder.scope("DirectionalLight[blur]");

            encoder
                .scoped_render_pass(
                    "DirectionalLight[blur][horizontal]",
                    wgpu::RenderPassDescriptor {
                        label: Some("DirectionalLight[blur][horizontal]"),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.temp_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        ..Default::default()
                    },
                )
                .execute_bundles(std::iter::once(&self.h_pass));

            encoder
                .scoped_render_pass(
                    "DirectionalLight[blur][vertical]",
                    wgpu::RenderPassDescriptor {
                        label: Some("DirectionalLight[blur][vertical]"),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.output_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        ..Default::default()
                    },
                )
                .execute_bundles(std::iter::once(&self.v_pass));
        }
    }
}
