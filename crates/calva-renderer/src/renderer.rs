use anyhow::{anyhow, Result};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::camera::Camera;
use crate::egui::EguiRenderer;
use crate::gbuffer::GeometryBuffer;

pub trait Renderable {
    fn render<'a: 'r, 'r>(&'a self, renderer: &'a Renderer, rpass: &mut wgpu::RenderPass<'r>);
}

pub struct ShaderGlobals {
    pub value: f32,

    buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl ShaderGlobals {
    fn new(device: &wgpu::Device) -> Self {
        let value = 0.0;

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shader globals uniform buffer"),
            contents: bytemuck::cast_slice(&[value]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shader globals uniform bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shader globals uniform bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            value,

            buffer,
            bind_group_layout,
            bind_group,
        }
    }

    fn update_buffer(&self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.value]));
    }
}

struct AmbientRenderer {
    pipeline: wgpu::RenderPipeline,
}

impl AmbientRenderer {
    fn new(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        globals: &ShaderGlobals,
        camera: &Camera,
        gbuffer: &GeometryBuffer,
    ) -> Self {
        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Ambient shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ambient.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Ambient pipeline layout"),
            bind_group_layouts: &[
                &globals.bind_group_layout,
                &camera.bind_group_layout,
                &gbuffer.bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Ambient pipeline"),
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
        });

        Self { pipeline }
    }

    fn render(
        &self,
        renderer: &Renderer,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
    ) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Ambient pass"),
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

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &renderer.globals.bind_group, &[]);
        rpass.set_bind_group(1, &renderer.camera.bind_group, &[]);
        rpass.set_bind_group(2, &renderer.gbuffer.bind_group, &[]);

        rpass.draw(0..6, 0..1);
    }
}

struct LightsRenderer {
    instances_buffer: wgpu::Buffer,
    positions_buffer: wgpu::Buffer,
    num_elements: u32,
    indices_buffer: wgpu::Buffer,
    stencil_pipeline: wgpu::RenderPipeline,
    lights_pipeline: wgpu::RenderPipeline,
}

impl LightsRenderer {
    fn new(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        globals: &ShaderGlobals,
        camera: &Camera,
        gbuffer: &GeometryBuffer,
    ) -> Self {
        let icosphere = crate::icosphere::Icosphere::new(1);

        let instances_data: &[f32] = &[
            // light 1
            1.0, 3.0, 1.0, // Position
            1.0, // Radius
            1.0, 0.0, 0.0, // Color
            // light 2
            1.0, 2.0, 1.0, // Position
            1.0, // Radius
            0.0, 1.0, 0.0, // Color
        ];
        let instances_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light instances buffer"),
            contents: bytemuck::cast_slice(instances_data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let positions_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light positions buffer"),
            contents: bytemuck::cast_slice(&icosphere.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light indices buffer"),
            contents: bytemuck::cast_slice(&icosphere.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let num_elements = icosphere.indices.len() as u32;

        let stencil_pipeline = {
            let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("Light stencil shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light_stencil.wgsl").into()),
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Light stencil pipeline layout"),
                bind_group_layouts: &[
                    &globals.bind_group_layout,
                    &camera.bind_group_layout,
                    &gbuffer.bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Light stencil pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "main",
                    buffers: &[
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 7) as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![
                                0 => Float32x3, // Position
                                1 => Float32, // Radius
                                2 => Float32x3, // Color
                            ],
                        },
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 3) as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![3 => Float32x3],
                        },
                    ],
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    clamp_depth: false,
                    // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Renderer::DEPTH_FORMAT,
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
                        read_mask: 0,
                        write_mask: 0xFF,
                    },
                    bias: wgpu::DepthBiasState {
                        constant: 0,
                        slope_scale: 0.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
            })
        };

        let lights_pipeline = {
            let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("Light shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light.wgsl").into()),
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Light pipeline layout"),
                bind_group_layouts: &[
                    &globals.bind_group_layout,
                    &camera.bind_group_layout,
                    &gbuffer.bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Light pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "main",
                    buffers: &[
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 7) as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![
                                0 => Float32x3, // Position
                                1 => Float32, // Radius
                                2 => Float32x3, // Color
                            ],
                        },
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<f32>() * 3) as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![3 => Float32x3],
                        },
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "main",
                    targets: &[wgpu::ColorTargetState {
                        format: surface_config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Front),
                    clamp_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Renderer::DEPTH_FORMAT,
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
                        write_mask: 0,
                    },
                    bias: wgpu::DepthBiasState {
                        constant: 0,
                        slope_scale: 0.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
            })
        };

        Self {
            instances_buffer,
            positions_buffer,
            num_elements,
            indices_buffer,
            stencil_pipeline,
            lights_pipeline,
        }
    }

    fn render(
        &self,
        renderer: &Renderer,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
    ) {
        // Lights stencil
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Light stencil pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &renderer.gbuffer.depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: true,
                    }),
                }),
            });

            rpass.set_pipeline(&self.stencil_pipeline);
            rpass.set_bind_group(0, &renderer.globals.bind_group, &[]);
            rpass.set_bind_group(1, &renderer.camera.bind_group, &[]);
            rpass.set_bind_group(2, &renderer.gbuffer.bind_group, &[]);

            rpass.set_vertex_buffer(0, self.instances_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.positions_buffer.slice(..));
            rpass.set_index_buffer(self.indices_buffer.slice(..), wgpu::IndexFormat::Uint16);

            rpass.draw_indexed(0..self.num_elements, 0, 0..2);
        }

        // Lights
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Light pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &renderer.gbuffer.depth.view,
                    // depth_ops: None,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    }),
                }),
            });

            rpass.set_pipeline(&self.lights_pipeline);
            rpass.set_bind_group(0, &renderer.globals.bind_group, &[]);
            rpass.set_bind_group(1, &renderer.camera.bind_group, &[]);
            rpass.set_bind_group(2, &renderer.gbuffer.bind_group, &[]);

            rpass.set_vertex_buffer(0, self.instances_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.positions_buffer.slice(..));

            rpass.set_index_buffer(self.indices_buffer.slice(..), wgpu::IndexFormat::Uint16);

            rpass.draw_indexed(0..self.num_elements, 0, 0..2);
        }
    }
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,

    pub globals: ShaderGlobals,
    pub camera: Camera,

    gbuffer: GeometryBuffer,
    ambient_renderer: AmbientRenderer,
    lights_renderer: LightsRenderer,

    pub egui: EguiRenderer,
}

impl Renderer {
    pub const ALBEDO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
    pub const POSITION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
    pub const NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

    pub const RENDER_TARGETS: &'static [wgpu::ColorTargetState] = &[
        wgpu::ColorTargetState {
            format: Self::ALBEDO_FORMAT,
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        },
        wgpu::ColorTargetState {
            format: Self::POSITION_FORMAT,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        },
        wgpu::ColorTargetState {
            format: Self::NORMAL_FORMAT,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        },
    ];

    pub const DEPTH_STENCIL: wgpu::DepthStencilState = wgpu::DepthStencilState {
        format: Self::DEPTH_FORMAT,
        depth_write_enabled: true,
        depth_compare: wgpu::CompareFunction::Less,
        stencil: wgpu::StencilState {
            front: wgpu::StencilFaceState::IGNORE,
            back: wgpu::StencilFaceState::IGNORE,
            read_mask: 0,
            write_mask: 0,
        },
        bias: wgpu::DepthBiasState {
            constant: 0,
            slope_scale: 0.0,
            clamp: 0.0,
        },
    };

    pub const MULTISAMPLE: wgpu::MultisampleState = wgpu::MultisampleState {
        count: 1,
        mask: !0,
        alpha_to_coverage_enabled: false,
    };

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

        let globals = ShaderGlobals::new(&device);
        let camera = Camera::new(&device);

        let gbuffer = GeometryBuffer::new(&device, &surface_config);

        let ambient_renderer =
            AmbientRenderer::new(&device, &surface_config, &globals, &camera, &gbuffer);

        let lights_renderer =
            LightsRenderer::new(&device, &surface_config, &globals, &camera, &gbuffer);

        let egui = EguiRenderer::new(&window, &device);

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,

            globals,
            camera,

            gbuffer,

            ambient_renderer,
            lights_renderer,

            egui,
        })
    }

    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.surface_config.width = size.width;
        self.surface_config.height = size.height;
        self.surface.configure(&self.device, &self.surface_config);

        self.gbuffer = GeometryBuffer::new(&self.device, &self.surface_config);
    }

    pub fn render<'a: 'r, 'r>(
        &'a mut self,
        window: &Window,
        renderables: impl IntoIterator<Item = &'r Box<dyn Renderable>>,
        app: &mut dyn epi::App,
    ) -> Result<(), wgpu::SurfaceError> {
        let output_frame = self.surface.get_current_texture()?;
        let output_view = output_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render encoder"),
            });

        self.camera.update_buffer(&self.queue);
        self.globals.update_buffer(&self.queue);

        {
            let mut geometry_pass = self.gbuffer.begin_render_pass(&mut encoder);
            for renderable in renderables {
                renderable.render(&self, &mut geometry_pass);
            }
        }

        self.ambient_renderer
            .render(&self, &mut encoder, &output_view);

        self.lights_renderer
            .render(&self, &mut encoder, &output_view);

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
