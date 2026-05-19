use std::cell::RefCell;

use anyhow::{anyhow, Result};

use wgpu_profiler::{GpuProfiler, GpuProfilerSettings, GpuTimerQueryResult};

use crate::{Resource, ResourcesManager};

pub struct Renderer {
    pub resources: ResourcesManager,

    #[allow(dead_code)]
    adapter: wgpu::Adapter,
    adapter_info: wgpu::AdapterInfo,

    profiler_results: RefCell<Vec<GpuTimerQueryResult>>,
}

impl Renderer {
    const FEATURES: wgpu::Features = wgpu::Features::empty()
        .union(wgpu::Features::DEPTH_CLIP_CONTROL) // all platforms
        .union(wgpu::Features::MULTI_DRAW_INDIRECT_COUNT) // Vulkan, DX12
        .union(wgpu::Features::INDIRECT_FIRST_INSTANCE) // Vulkan, DX12, Metal
        .union(wgpu::Features::TEXTURE_BINDING_ARRAY) // Vulkan, DX12, Metal
        .union(wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY) // Vulkan, Metal
        .union(wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING) // Vulkan, DX12, Metal
        .union(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES) // All except WebGL
        .union(wgpu::Features::POLYGON_MODE_LINE) // Vulkan, DX12, Metal
        .union(wgpu::Features::FLOAT32_FILTERABLE) // Vulkan, DX12, Metal
        .union(wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY) // Vulkan, DX12
        .union(wgpu::Features::DEPTH32FLOAT_STENCIL8) // Vulkan, DX12, Metal
        .union(wgpu::Features::IMMEDIATES) // Vulkan, DX12, Metal
        .union(GpuProfiler::ALL_WGPU_TIMER_FEATURES) // Vulkan, DX12
        ;

    pub async fn new(
        display: Box<dyn wgpu::wgt::WgpuHasDisplayHandle>,
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        size: (u32, u32),
    ) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_with_display_handle_from_env(display)
        });
        let surface = instance.create_surface(window)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;

        let adapter_info = adapter.get_info();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Renderer device"),
                required_features: Self::FEATURES,
                required_limits: wgpu::Limits {
                    max_sampled_textures_per_shader_stage: 512,
                    max_binding_array_elements_per_shader_stage: 512,
                    max_immediate_size: 128,
                    max_bind_groups: 6,
                    ..Default::default()
                },
                ..Default::default()
            })
            .await?;

        let mut surface_config = surface
            .get_default_config(&adapter, size.0, size.1)
            .ok_or_else(|| anyhow!("Surface not compatible with adapter"))?;
        surface_config.format = surface_config.format.add_srgb_suffix();
        surface_config.present_mode = wgpu::PresentMode::AutoNoVsync;
        // surface_config.present_mode = wgpu::PresentMode::AutoVsync;
        // surface_config.present_mode = wgpu::PresentMode::Fifo;

        surface.configure(&device, &surface_config);

        let profiler_results = RefCell::new(vec![]);

        let resources = ResourcesManager::new(device, queue, surface, surface_config);

        Ok(Self {
            adapter,
            adapter_info,

            resources,

            profiler_results,
        })
    }

    pub fn render(&self, cb: impl FnOnce(&mut RenderContext) -> Result<()>) -> Result<()> {
        let device = self.resources.read::<wgpu::Device>();
        let queue = self.resources.read::<wgpu::Queue>();
        let surface = self.resources.read::<wgpu::Surface>();
        let surface_config = self.resources.read::<wgpu::SurfaceConfiguration>();
        let mut profiler = self.resources.write::<GpuProfiler>();

        let mut encoder = device.create_command_encoder(&Default::default());

        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(_) => {
                surface.configure(&device, &surface_config);
                return Ok(());
            }

            _ => return Err(anyhow!("Failed")),
        };

        let scope = profiler.scope("RenderPass", &mut encoder);

        let mut context = RenderContext {
            encoder: scope,
            frame: &frame.texture.create_view(&Default::default()),
        };

        cb(&mut context)?;
        drop(context);

        profiler.resolve_queries(&mut encoder);

        let submission_index = queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission_index),
            timeout: None,
        })?;

        profiler.end_frame()?;
        if let Some(results) = profiler.process_finished_frame(queue.get_timestamp_period()) {
            *self.profiler_results.try_borrow_mut()? = results;
        }

        Ok(())
    }
}

#[cfg(feature = "egui")]
impl egui::Widget for &Renderer {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical(|ui| {
            egui::CollapsingHeader::new("Adapter")
                .default_open(true)
                .show(ui, |ui| {
                    let wgpu::AdapterInfo {
                        name,
                        driver,
                        driver_info,
                        ..
                    } = &self.adapter_info;

                    egui::Grid::new("EguiPass::AdapterInfo")
                        .num_columns(2)
                        .spacing([40.0, 0.0])
                        .show(ui, |ui| {
                            ui.label("Device");
                            ui.label(name);

                            ui.end_row();

                            ui.label("Driver");
                            ui.label(format!("{driver} ({driver_info})"));
                        });
                });

            let profiler = self.resources.read::<GpuProfiler>();
            if !profiler.settings().enable_timer_queries {
                return;
            }

            fn profiler_ui(results: &[GpuTimerQueryResult]) -> impl FnOnce(&mut egui::Ui) + '_ {
                move |ui| {
                    let frame = egui::Frame {
                        inner_margin: egui::Margin {
                            left: 10,
                            ..Default::default()
                        },
                        ..Default::default()
                    };

                    for result in results {
                        ui.vertical(|ui| {
                            ui.columns(2, |columns| {
                                columns[0].label(&result.label);

                                columns[1].with_layout(
                                    egui::Layout::right_to_left(egui::Align::TOP),
                                    |ui| {
                                        let start = result
                                            .time
                                            .as_ref()
                                            .map(|r| r.start)
                                            .unwrap_or_default();
                                        let end =
                                            result.time.as_ref().map(|r| r.end).unwrap_or_default();

                                        let diff = end - start;
                                        let time = diff * 1000.0 * 1000.0;
                                        let time_str = format!("{time:.3}");
                                        ui.monospace(format!("{time_str} µs"));
                                    },
                                )
                            });

                            frame.show(ui, profiler_ui(&result.nested_queries));
                        });
                    }
                }
            }

            egui::CollapsingHeader::new("Profiler")
                .default_open(true)
                .show(ui, profiler_ui(&self.profiler_results.borrow()));
        })
        .response
    }
}

pub type ProfilerCommandEncoder<'a> = wgpu_profiler::Scope<'a, wgpu::CommandEncoder>;

pub struct RenderContext<'a> {
    pub encoder: ProfilerCommandEncoder<'a>,
    pub frame: &'a wgpu::TextureView,
}

impl Resource for GpuProfiler {
    fn instanciate(resources: &ResourcesManager) -> Result<Self> {
        let device = resources.read::<wgpu::Device>();

        let profiler = GpuProfiler::new(
            &device,
            GpuProfilerSettings {
                // enable_debug_groups: false,
                // enable_timer_queries: false,
                ..Default::default()
            },
        )?;

        Ok(profiler)
    }
}
