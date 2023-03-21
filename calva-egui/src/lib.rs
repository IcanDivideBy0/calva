#![warn(clippy::all)]

use egui::ClippedPrimitive;
use renderer::{
    wgpu, AmbientLightConfig, Engine, RenderContext, Renderer, SsaoConfig, ToneMappingConfig,
};

#[cfg(feature = "profiler")]
use thousands::Separable;

#[cfg(feature = "winit")]
use winit::event_loop::EventLoop;

pub use egui;

pub struct EguiPass {
    pub context: egui::Context,

    paint_jobs: Vec<ClippedPrimitive>,
    screen_descriptor: egui_wgpu::renderer::ScreenDescriptor,
    egui_renderer: egui_wgpu::Renderer,
}

impl EguiPass {
    pub fn new(renderer: &Renderer) -> Self {
        let egui_renderer =
            egui_wgpu::Renderer::new(&renderer.device, renderer.surface_config.format, None, 1);

        let screen_descriptor = egui_wgpu::renderer::ScreenDescriptor {
            size_in_pixels: [
                renderer.surface_config.width,
                renderer.surface_config.height,
            ],
            pixels_per_point: 1.0,
        };

        Self {
            context: Default::default(),
            paint_jobs: vec![],
            screen_descriptor,
            egui_renderer,
        }
    }

    pub fn run(&self, input: egui::RawInput, ui: impl FnOnce(&egui::Context)) -> egui::FullOutput {
        self.context.run(input, ui)
    }

    pub fn update(
        &mut self,
        renderer: &Renderer,
        shapes: Vec<egui::epaint::ClippedShape>,
        textures_delta: egui::TexturesDelta,
    ) {
        self.screen_descriptor = egui_wgpu::renderer::ScreenDescriptor {
            size_in_pixels: [
                renderer.surface_config.width,
                renderer.surface_config.height,
            ],
            pixels_per_point: 1.0,
        };

        self.paint_jobs = self.context.tessellate(shapes);

        for (texture_id, image_delta) in &textures_delta.set {
            self.egui_renderer.update_texture(
                &renderer.device,
                &renderer.queue,
                *texture_id,
                image_delta,
            );
        }
        for texture_id in &textures_delta.free {
            self.egui_renderer.free_texture(texture_id);
        }

        let mut encoder = renderer.device.create_command_encoder(&Default::default());
        self.egui_renderer.update_buffers(
            &renderer.device,
            &renderer.queue,
            &mut encoder,
            &self.paint_jobs,
            &self.screen_descriptor,
        );
        renderer.queue.submit(std::iter::once(encoder.finish()));
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        self.egui_renderer.render(
            &mut ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: ctx.frame,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            }),
            &self.paint_jobs,
            &self.screen_descriptor,
        );
    }

    pub fn renderer_ui<'r: 'ui, 'ui>(renderer: &'r Renderer) -> impl FnOnce(&mut egui::Ui) + 'ui {
        move |ui| {
            egui::CollapsingHeader::new("Adapter")
                .default_open(true)
                .show(ui, EguiPass::adapter_info_ui(&renderer.adapter_info));

            #[cfg(feature = "profiler")]
            egui::CollapsingHeader::new("Profiler")
                .default_open(true)
                .show(ui, EguiPass::profiler_ui(&renderer.profiler_results()));
        }
    }

    pub fn adapter_info_ui(adapter_info: &wgpu::AdapterInfo) -> impl FnOnce(&mut egui::Ui) + '_ {
        let wgpu::AdapterInfo {
            name,
            driver,
            driver_info,
            ..
        } = adapter_info;

        move |ui| {
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
        }
    }

    #[cfg(feature = "profiler")]
    pub fn profiler_ui(results: &[renderer::ProfilerResult]) -> impl FnOnce(&mut egui::Ui) + '_ {
        move |ui| {
            let frame = egui::Frame {
                inner_margin: egui::style::Margin {
                    left: 10.0,
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
                                let time = (result.time.end - result.time.start) * 1000.0 * 1000.0;
                                let time_str = format!("{time:.3}").separate_with_commas();
                                ui.monospace(format!("{time_str} µs"));
                            },
                        )
                    });

                    frame.show(ui, Self::profiler_ui(&result.nested_scopes));
                });
            }
        }
    }

    pub fn engine_config_ui<'e: 'ui, 'ui>(
        engine: &'e mut Engine,
    ) -> impl FnOnce(&mut egui::Ui) + 'ui {
        move |ui| {
            egui::CollapsingHeader::new("Ambient")
                .default_open(true)
                .show(ui, EguiPass::ambient_config_ui(&mut engine.config.ambient));

            egui::CollapsingHeader::new("SSAO")
                .default_open(true)
                .show(ui, EguiPass::ssao_config_ui(&mut engine.config.ssao));

            egui::CollapsingHeader::new("Tone mapping")
                .default_open(true)
                .show(
                    ui,
                    EguiPass::tone_mapping_config_ui(&mut engine.config.tone_mapping),
                );
        }
    }

    pub fn ambient_config_ui<'cfg: 'ui, 'ui>(
        config: &'cfg mut AmbientLightConfig,
    ) -> impl FnOnce(&mut egui::Ui) + 'ui {
        move |ui| {
            ui.add(egui::Slider::new(&mut config.factor, 0.0..=1.0).text("Factor"));
        }
    }

    pub fn ssao_config_ui<'cfg: 'ui, 'ui>(
        config: &'cfg mut SsaoConfig,
    ) -> impl FnOnce(&mut egui::Ui) + 'ui {
        move |ui| {
            ui.add(egui::Slider::new(&mut config.radius, 0.0..=4.0).text("Radius"));
            ui.add(egui::Slider::new(&mut config.bias, 0.0..=0.1).text("Bias"));
            ui.add(egui::Slider::new(&mut config.power, 0.0..=8.0).text("Power"));
        }
    }

    pub fn tone_mapping_config_ui<'cfg: 'ui, 'ui>(
        config: &'cfg mut ToneMappingConfig,
    ) -> impl FnOnce(&mut egui::Ui) + 'ui {
        move |ui| {
            ui.add(
                egui::Slider::new(&mut config.exposure, 0.0..=10.0)
                    .text("Exposure")
                    .logarithmic(true),
            );
            ui.add(egui::Slider::new(&mut config.gamma, 1.0..=3.0).text("Gamma"));
        }
    }
}

#[cfg(feature = "winit")]
pub struct EguiWinitPass {
    pass: EguiPass,
    state: egui_winit::State,
}

#[cfg(feature = "winit")]
impl EguiWinitPass {
    pub fn new(renderer: &Renderer, event_loop: &EventLoop<()>) -> Self {
        Self {
            pass: EguiPass::new(renderer),
            state: egui_winit::State::new(event_loop),
        }
    }

    pub fn on_event(&mut self, event: &winit::event::WindowEvent) -> egui_winit::EventResponse {
        self.state.on_event(&self.pass.context, event)
    }

    pub fn update(
        &mut self,
        renderer: &Renderer,
        window: &winit::window::Window,
        ui: impl FnOnce(&egui::Context),
    ) {
        let output = self.pass.run(self.state.take_egui_input(window), ui);

        self.state
            .handle_platform_output(window, &self.pass.context, output.platform_output);
        self.pass
            .update(renderer, output.shapes, output.textures_delta);
    }
}

#[cfg(feature = "winit")]
impl std::ops::Deref for EguiWinitPass {
    type Target = EguiPass;

    fn deref(&self) -> &Self::Target {
        &self.pass
    }
}
