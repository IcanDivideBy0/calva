use crate::{
    AnimationState, AnimationsManager, CameraManager, DirectionalLight, GeometryPass,
    InstancesManager, MaterialId, MeshesManager, RenderContext, Renderer, SkinsManager,
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

pub struct DirectionalLightPass {
    uniform: DirectionalLightUniform,
    cull: DirectionalLightCull,

    sampler: wgpu::Sampler,
    depth: wgpu::TextureView,
    depth_pipeline: wgpu::RenderPipeline,

    blur_pass: DirectionalLightBlur,

    shadow_bind_group_layout: wgpu::BindGroupLayout,
    shadow_bind_group: wgpu::BindGroup,
    shadow_pipeline: wgpu::RenderPipeline,
}

impl DirectionalLightPass {
    const SIZE: u32 = 2048;
    const TEXTURE_SIZE: wgpu::Extent3d = wgpu::Extent3d {
        width: Self::SIZE,
        height: Self::SIZE,
        depth_or_array_layers: 1,
    };

    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(
        renderer: &Renderer,
        camera: &CameraManager,
        meshes: &MeshesManager,
        skins: &SkinsManager,
        animations: &AnimationsManager,
        instances: &InstancesManager,
        geometry: &GeometryPass,
    ) -> Self {
        let uniform = DirectionalLightUniform::new(renderer);

        let cull = DirectionalLightCull::new(renderer, camera, meshes, instances, &uniform);

        let sampler = renderer.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("DirectionalLight sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let depth = Self::make_depth_texture(renderer, Some("DirectionalLight depth texture"))
            .create_view(&Default::default());

        let depth_pipeline = {
            let shader = renderer
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("DirectionalLight[depth] shader"),
                    source: wgpu::ShaderSource::Wgsl(
                        include_str!("directional_light.depth.wgsl").into(),
                    ),
                });

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("DirectionalLight[depth] render pipeline layout"),
                        bind_group_layouts: &[
                            &uniform.bind_group_layout,
                            &skins.bind_group_layout,
                            &animations.bind_group_layout,
                        ],
                        push_constant_ranges: &[wgpu::PushConstantRange {
                            stages: wgpu::ShaderStages::VERTEX,
                            range: 0..(std::mem::size_of::<f32>() as _),
                        }],
                    });

            renderer
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("DirectionalLight[depth] render pipeline"),
                    layout: Some(&pipeline_layout),
                    multiview: None,
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_main",
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
                        format: Self::DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                })
        };

        let blur_pass = blur::DirectionalLightBlur::new(renderer, &depth);

        let (shadow_bind_group_layout, shadow_bind_group, shadow_pipeline) = {
            let shader = renderer
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("DirectionalLight[shadow] shader"),
                    source: wgpu::ShaderSource::Wgsl(
                        include_str!("directional_light.shadow.wgsl").into(),
                    ),
                });

            let bind_group_layout =
                renderer
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("DirectionalLight[shadow] bind group layout"),
                        entries: &[
                            // albedo + metallic
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: false,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
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
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
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

            let bind_group = Self::make_shadow_bind_group(
                renderer,
                geometry,
                &bind_group_layout,
                &depth,
                &sampler,
            );

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("DirectionalLight[shadow] pipeline layout"),
                        bind_group_layouts: &[
                            &camera.bind_group_layout,
                            &uniform.bind_group_layout,
                            &bind_group_layout,
                        ],
                        push_constant_ranges: &[wgpu::PushConstantRange {
                            stages: wgpu::ShaderStages::FRAGMENT,
                            range: 0..(std::mem::size_of::<f32>() as _),
                        }],
                    });

            let pipeline =
                renderer
                    .device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("DirectionalLight[shadow] pipeline"),
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
                                format: renderer.surface_config.format,
                                blend: Some(wgpu::BlendState {
                                    color: wgpu::BlendComponent {
                                        src_factor: wgpu::BlendFactor::One,
                                        dst_factor: wgpu::BlendFactor::One,
                                        operation: wgpu::BlendOperation::Add,
                                    },
                                    alpha: wgpu::BlendComponent {
                                        src_factor: wgpu::BlendFactor::One,
                                        dst_factor: wgpu::BlendFactor::One,
                                        operation: wgpu::BlendOperation::Max,
                                    },
                                }),
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        primitive: Default::default(),
                        depth_stencil: None,
                        multisample: Default::default(),
                    });

            (bind_group_layout, bind_group, pipeline)
        };

        Self {
            uniform,
            cull,

            sampler,
            depth,
            depth_pipeline,

            blur_pass,

            shadow_bind_group_layout,
            shadow_bind_group,
            shadow_pipeline,
        }
    }

    pub fn rebind(&mut self, renderer: &Renderer, geometry: &GeometryPass) {
        self.shadow_bind_group = Self::make_shadow_bind_group(
            renderer,
            geometry,
            &self.shadow_bind_group_layout,
            &self.depth,
            &self.sampler,
        );
    }

    pub fn update(
        &self,
        renderer: &Renderer,
        camera: &CameraManager,
        directional_light: &DirectionalLight,
    ) {
        self.uniform.update(renderer, camera, directional_light);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        ctx: &mut RenderContext,
        camera: &CameraManager,
        meshes: &MeshesManager,
        skins: &SkinsManager,
        animations: &AnimationsManager,
        instances: &InstancesManager,
        gamma: f32,
    ) {
        ctx.encoder.profile_start("DirectionalLight");

        self.cull
            .cull(ctx, camera, meshes, instances, &self.uniform);

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("DirectionalLight[depth]"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        rpass.set_pipeline(&self.depth_pipeline);

        rpass.set_bind_group(0, &self.uniform.bind_group, &[]);
        rpass.set_bind_group(1, &skins.bind_group, &[]);
        rpass.set_bind_group(2, &animations.bind_group, &[]);

        rpass.set_vertex_buffer(0, self.cull.draw_instances.slice(..));
        rpass.set_vertex_buffer(1, meshes.vertices.slice(..));

        rpass.set_index_buffer(meshes.indices.slice(..), wgpu::IndexFormat::Uint32);

        rpass.multi_draw_indexed_indirect_count(
            &self.cull.draw_indirects,
            std::mem::size_of::<u32>() as _,
            &self.cull.draw_indirects,
            0,
            MeshesManager::MAX_MESHES as _,
        );

        drop(rpass);

        self.blur_pass.render(ctx, &self.depth);

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("DirectionalLight[shadow]"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        rpass.set_pipeline(&self.shadow_pipeline);

        rpass.set_bind_group(0, &camera.bind_group, &[]);
        rpass.set_bind_group(1, &self.uniform.bind_group, &[]);
        rpass.set_bind_group(2, &self.shadow_bind_group, &[]);

        rpass.set_push_constants(wgpu::ShaderStages::FRAGMENT, 0, bytemuck::bytes_of(&gamma));

        rpass.draw(0..3, 0..1);

        drop(rpass);

        ctx.encoder.profile_end();
    }

    fn make_shadow_bind_group(
        renderer: &Renderer,
        geometry: &GeometryPass,
        layout: &wgpu::BindGroupLayout,
        depth: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("DirectionalLight[shadow] bind group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&geometry.albedo_metallic),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&geometry.normal_roughness),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&renderer.depth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(depth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
    }

    fn make_depth_texture(renderer: &Renderer, label: wgpu::Label<'static>) -> wgpu::Texture {
        renderer.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: Self::TEXTURE_SIZE,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[Self::DEPTH_FORMAT],
        })
    }
}

use uniform::*;
mod uniform {
    use crate::{CameraManager, DirectionalLight, DirectionalLightPass, Renderer};

    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
    struct DirectionalLightUniformRaw {
        color: glam::Vec4,
        direction_world: glam::Vec4,
        direction_view: glam::Vec4,
        view_proj: glam::Mat4,
    }

    pub struct DirectionalLightUniform {
        buffer: wgpu::Buffer,

        pub(crate) bind_group_layout: wgpu::BindGroupLayout,
        pub(crate) bind_group: wgpu::BindGroup,
    }

    impl DirectionalLightUniform {
        pub fn new(renderer: &Renderer) -> Self {
            let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("DirectionalLight uniform"),
                size: std::mem::size_of::<DirectionalLightUniformRaw>() as _,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let bind_group_layout =
                renderer
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("DirectionalLight uniform bind group layout"),
                        entries: &[wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                | wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new({
                                    std::mem::size_of::<DirectionalLightUniformRaw>() as _
                                }),
                            },
                            count: None,
                        }],
                    });

            let bind_group = renderer
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("DirectionalLight bind group"),
                    layout: &bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    }],
                });

            Self {
                buffer,
                bind_group_layout,
                bind_group,
            }
        }

        pub fn update(
            &self,
            renderer: &Renderer,
            camera: &CameraManager,
            directional_light: &DirectionalLight,
        ) {
            let light_dir = directional_light.direction.normalize();
            let light_view = glam::Mat4::look_at_rh(glam::Vec3::ZERO, light_dir, glam::Vec3::Y);

            // Frustum bounding sphere in view space
            // https://lxjk.github.io/2017/04/15/Calculate-Minimal-Bounding-Sphere-of-Frustum.html
            // https://stackoverflow.com/questions/2194812/finding-a-minimum-bounding-sphere-for-a-frustum
            // https://stackoverflow.com/questions/56428880/how-to-extract-camera-parameters-from-projection-matrix
            let proj = camera.proj;
            let znear = proj.w_axis.z / (proj.z_axis.z - 1.0);
            let zfar = proj.w_axis.z / (proj.z_axis.z + 1.0);

            let k =
                f32::sqrt(1.0 + (proj.x_axis.x / proj.y_axis.y).powi(2)) * proj.x_axis.x.recip();
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
            center = (light_view * camera.view.inverse() * center.extend(1.0)).truncate();

            // Avoid shadow swimming:
            // Prevent small radius changes due to float precision
            radius = (radius * 16.0).ceil() / 16.0;
            // Shadow texel size in light view space
            let texel_size = radius * 2.0 / DirectionalLightPass::SIZE as f32;
            // Allow center changes only in texel size increments
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

            renderer.queue.write_buffer(
                &self.buffer,
                0,
                bytemuck::bytes_of(&DirectionalLightUniformRaw {
                    color: directional_light.color,
                    direction_world: light_dir.extend(0.0),
                    direction_view: (glam::Quat::from_mat4(&camera.view) * light_dir).extend(0.0),
                    view_proj: (light_proj * light_view),
                }),
            );
        }
    }
}

use cull::*;
mod cull {
    use crate::{
        CameraManager, Instance, InstancesManager, MeshInfo, MeshesManager, RenderContext, Renderer,
    };

    use super::{uniform::DirectionalLightUniform, DrawInstance};

    pub struct DirectionalLightCull {
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
            renderer: &Renderer,
            camera: &CameraManager,
            meshes: &MeshesManager,
            instances: &InstancesManager,
            uniform: &DirectionalLightUniform,
        ) -> Self {
            let draw_instances = renderer.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("DirectionalLight[cull] draw instances"),
                size: (std::mem::size_of::<[DrawInstance; InstancesManager::MAX_INSTANCES]>()) as _,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            });

            let draw_indirects = renderer.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("DirectionalLight[cull] draw indirects"),
                size: {
                    let count_size = std::mem::size_of::<u32>();
                    let indirects_size = std::mem::size_of::<
                        [wgpu::util::DrawIndexedIndirect; MeshesManager::MAX_MESHES],
                    >();

                    count_size + indirects_size
                } as _,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::INDIRECT,
                mapped_at_creation: false,
            });

            let bind_group_layout =
                renderer
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                                        std::mem::size_of::<u32>() as _,
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
                                            + std::mem::size_of::<wgpu::util::DrawIndexedIndirect>()
                                                as u64,
                                    ),
                                },
                                count: None,
                            },
                        ],
                    });

            let bind_group = renderer
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("DirectionalLight[cull] bind group"),
                    layout: &bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: meshes.meshes_info.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: instances.base_instances.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: instances.instances.as_entire_binding(),
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

            let shader = renderer
                .device
                .create_shader_module(wgpu::include_wgsl!("directional_light.cull.wgsl"));

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("DirectionalLight[cull] pipeline layout"),
                        bind_group_layouts: &[
                            &camera.bind_group_layout,
                            &uniform.bind_group_layout,
                            &bind_group_layout,
                        ],
                        push_constant_ranges: &[wgpu::PushConstantRange {
                            stages: wgpu::ShaderStages::COMPUTE,
                            range: 0..(std::mem::size_of::<u32>() as _),
                        }],
                    });

            let pipelines = (
                renderer
                    .device
                    .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some("DirectionalLight[cull] reset pipeline"),
                        layout: Some(&pipeline_layout),
                        module: &shader,
                        entry_point: "reset",
                    }),
                renderer
                    .device
                    .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some("DirectionalLight[cull] cull pipeline"),
                        layout: Some(&pipeline_layout),
                        module: &shader,
                        entry_point: "cull",
                    }),
                renderer
                    .device
                    .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some("DirectionalLight[cull] count pipeline"),
                        layout: Some(&pipeline_layout),
                        module: &shader,
                        entry_point: "count",
                    }),
            );

            Self {
                draw_instances,
                draw_indirects,

                bind_group,
                pipelines,
            }
        }

        pub fn cull(
            &self,
            ctx: &mut RenderContext,
            camera: &CameraManager,
            meshes: &MeshesManager,
            instances: &InstancesManager,
            uniform: &DirectionalLightUniform,
        ) {
            let mut cpass = ctx
                .encoder
                .begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("DirectionalLight[cull]"),
                });

            let meshes_count: u32 = meshes.count();
            let instances_count: u32 = instances.count();

            cpass.set_pipeline(&self.pipelines.0);
            cpass.set_bind_group(0, &camera.bind_group, &[]);
            cpass.set_bind_group(1, &uniform.bind_group, &[]);
            cpass.set_bind_group(2, &self.bind_group, &[]);
            cpass.dispatch_workgroups(meshes_count, 1, 1);

            cpass.set_pipeline(&self.pipelines.1);
            cpass.set_bind_group(0, &camera.bind_group, &[]);
            cpass.set_bind_group(1, &uniform.bind_group, &[]);
            cpass.set_bind_group(2, &self.bind_group, &[]);
            cpass.dispatch_workgroups(instances_count, 1, 1);

            cpass.set_pipeline(&self.pipelines.2);
            cpass.set_bind_group(0, &camera.bind_group, &[]);
            cpass.set_bind_group(1, &uniform.bind_group, &[]);
            cpass.set_bind_group(2, &self.bind_group, &[]);
            cpass.dispatch_workgroups(meshes_count, 1, 1);
        }
    }
}

use blur::*;
mod blur {
    use crate::{RenderContext, Renderer};

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
        temp: wgpu::TextureView,

        h_pass: wgpu::RenderBundle,
        v_pass: wgpu::RenderBundle,
    }

    impl DirectionalLightBlur {
        pub fn new(renderer: &Renderer, output: &wgpu::TextureView) -> Self {
            let temp = DirectionalLightPass::make_depth_texture(
                renderer,
                Some("DirectionalLightBlur temp texture"),
            )
            .create_view(&Default::default());

            let bind_group_layout =
                renderer
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("DirectionalLightBlur bind group layout"),
                        entries: &[wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Depth,
                            },
                            count: None,
                        }],
                    });

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("DirectionalLightBlur pipeline layout"),
                        bind_group_layouts: &[&bind_group_layout],
                        push_constant_ranges: &[],
                    });

            let shader = renderer
                .device
                .create_shader_module(wgpu::include_wgsl!("directional_light.blur.wgsl"));

            let make_render_bundle = |direction: Direction| {
                let bind_group = renderer
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some(
                            format!("DirectionalLightBlur[{direction}] bind group").as_str(),
                        ),
                        layout: &bind_group_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(match direction {
                                Direction::Horizontal => output,
                                Direction::Vertical => &temp,
                            }),
                        }],
                    });

                let pipeline =
                    renderer
                        .device
                        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                            label: Some(
                                format!("DirectionalLightBlur[{direction}] pipeline").as_str(),
                            ),
                            layout: Some(&pipeline_layout),
                            vertex: wgpu::VertexState {
                                module: &shader,
                                entry_point: "vs_main",
                                buffers: &[],
                            },
                            fragment: Some(wgpu::FragmentState {
                                module: &shader,
                                entry_point: format!("fs_main_{direction}").as_str(),
                                targets: &[],
                            }),
                            primitive: Default::default(),
                            depth_stencil: Some(wgpu::DepthStencilState {
                                format: DirectionalLightPass::DEPTH_FORMAT,
                                depth_write_enabled: true,
                                depth_compare: wgpu::CompareFunction::Always,
                                stencil: wgpu::StencilState::default(),
                                bias: wgpu::DepthBiasState::default(),
                            }),
                            multisample: Default::default(),
                            multiview: None,
                        });

                let mut encoder = renderer.device.create_render_bundle_encoder(
                    &wgpu::RenderBundleEncoderDescriptor {
                        label: Some(
                            format!("DirectionalLightBlur[{direction}] render bundle").as_str(),
                        ),
                        color_formats: &[],
                        depth_stencil: Some(wgpu::RenderBundleDepthStencil {
                            format: DirectionalLightPass::DEPTH_FORMAT,
                            depth_read_only: false,
                            stencil_read_only: false,
                        }),
                        sample_count: 1,
                        multiview: None,
                    },
                );

                encoder.set_pipeline(&pipeline);
                encoder.set_bind_group(0, &bind_group, &[]);

                encoder.draw(0..3, 0..1);

                encoder.finish(&Default::default())
            };

            let h_pass = make_render_bundle(Direction::Horizontal);
            let v_pass = make_render_bundle(Direction::Vertical);

            Self {
                temp,

                h_pass,
                v_pass,
            }
        }

        pub fn render(&self, ctx: &mut RenderContext, output: &wgpu::TextureView) {
            ctx.encoder.profile_start("DirectionalLight[blur]");

            ctx.encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("DirectionalLight[blur][horizontal]"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.temp,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                })
                .execute_bundles(std::iter::once(&self.h_pass));

            ctx.encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("DirectionalLight[blur][vertical]"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: output,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                })
                .execute_bundles(std::iter::once(&self.v_pass));

            ctx.encoder.profile_end();
        }
    }
}
