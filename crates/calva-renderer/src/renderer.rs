use anyhow::{anyhow, Result};
use winit::window::Window;

use super::camera::*;
use super::egui::EguiRenderer;
use super::model::*;
use super::texture::Texture;

pub(crate) struct GeometryBuffer {
    pub albedo: Texture,
    pub position: Texture,
    pub normal: Texture,
    pub depth: Texture,

    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl GeometryBuffer {
    pub fn new(device: &wgpu::Device, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let albedo = Texture::create_render_texture(
            device,
            surface_config,
            "GBuffer Albedo Texture",
            wgpu::TextureFormat::Bgra8UnormSrgb,
        );
        let position = Texture::create_render_texture(
            device,
            surface_config,
            "GBuffer Position Texture",
            wgpu::TextureFormat::Rgba32Float,
        );
        let normal = Texture::create_render_texture(
            device,
            surface_config,
            "GBuffer Normal Texture",
            wgpu::TextureFormat::Rgba32Float,
        );

        let depth = Texture::create_render_texture(
            device,
            surface_config,
            "GBuffer Normal Texture",
            wgpu::TextureFormat::Depth32Float,
        );

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GBuffer Bind GroupLayout"),
            entries: &[
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GBuffer Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&albedo.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&position.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&normal.view),
                },
            ],
        });

        Self {
            albedo,
            position,
            normal,
            depth,

            bind_group_layout,
            bind_group,
        }
    }

    pub fn begin_render_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> wgpu::RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Geometry Pass"),
            color_attachments: &[
                wgpu::RenderPassColorAttachment {
                    view: &self.albedo.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                },
                wgpu::RenderPassColorAttachment {
                    view: &self.position.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                },
                wgpu::RenderPassColorAttachment {
                    view: &self.normal.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                },
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        })
    }
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,

    pub(crate) camera_uniforms: CameraUniforms,
    pub(crate) gbuffer: GeometryBuffer,
    pub(crate) pipeline: wgpu::RenderPipeline,

    pub egui: EguiRenderer,
}

impl Renderer {
    pub async fn new(window: &Window) -> Result<Self> {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .ok_or(anyhow!("Cannot request WebGPU adapter"))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    label: None,
                },
                None, // Trace path
            )
            .await?;

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface
                .get_preferred_format(&adapter)
                .ok_or(anyhow!("Unable to get surface preferred format"))?,
            width: size.width as u32,
            height: size.height as u32,
            present_mode: wgpu::PresentMode::Immediate,
            // present_mode: wgpu::PresentMode::Mailbox,
            // present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &surface_config);

        let camera_uniforms = CameraUniforms::new(&device);

        let gbuffer = GeometryBuffer::new(&device, &surface_config);

        let pipeline = {
            let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("Renderer Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/splash_quad.wgsl").into()),
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Renderer Pipeline Layout"),
                bind_group_layouts: &[&gbuffer.bind_group_layout],
                push_constant_ranges: &[],
            });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Renderer Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "main",
                    targets: &[wgpu::ColorTargetState {
                        format: surface_config.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    clamp_depth: false,
                    // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
            })
        };

        let egui = EguiRenderer::new(&window, &device);

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,

            camera_uniforms,
            gbuffer,
            pipeline,

            egui,
        })
    }

    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.surface_config.width = size.width;
        self.surface_config.height = size.height;
        self.surface.configure(&self.device, &self.surface_config);

        self.gbuffer = GeometryBuffer::new(&self.device, &self.surface_config);
    }

    pub fn update_camera(&mut self, camera: &dyn Camera) {
        self.camera_uniforms.update(&self.queue, camera)
    }

    pub fn render(
        &mut self,
        window: &Window,
        models: &[Model],
        app: &mut dyn epi::App,
    ) -> Result<(), wgpu::SurfaceError> {
        let output_frame = self.surface.get_current_texture()?;
        let output_view = output_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut geometry_pass = self.gbuffer.begin_render_pass(&mut encoder);

            for model in models {
                for mesh in &model.meshes {
                    for primitive in &mesh.primitives {
                        let material = &model.materials[primitive.material];
                        geometry_pass.set_pipeline(&material.pipeline);

                        geometry_pass.set_bind_group(0, &self.camera_uniforms.bind_group, &[]);
                        geometry_pass.set_vertex_buffer(0, mesh.instances_buffer.slice(..));
                        geometry_pass.set_vertex_buffer(1, primitive.vertex_buffer.slice(..));
                        geometry_pass.set_index_buffer(
                            primitive.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );

                        geometry_pass.draw_indexed(
                            0..primitive.num_elements,
                            0,
                            0..mesh.instances.len() as u32,
                        );
                    }
                }
            }
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.gbuffer.bind_group, &[]);

            render_pass.draw(0..6, 0..1);
        }

        // egui
        self.egui
            .render(
                window,
                &self.device,
                &self.queue,
                &self.surface_config,
                &output_view,
                &mut encoder,
                app,
            )
            .expect("egui");

        self.queue.submit(std::iter::once(encoder.finish()));
        output_frame.present();

        Ok(())
    }
}
