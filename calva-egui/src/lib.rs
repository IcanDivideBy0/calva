#![warn(clippy::all)]

use renderer::{wgpu, AmbientConfig, Engine, ProfilerResult, RenderContext, Renderer, SsaoConfig};
use thousands::Separable;

pub use egui;

pub struct EguiPass {
    egui_renderer: egui_wgpu::Renderer,
}

impl EguiPass {
    pub fn new(renderer: &Renderer) -> Self {
        let egui_renderer = egui_wgpu::Renderer::new(
            &renderer.device,
            renderer.surface_config.format,
            Some(Renderer::DEPTH_FORMAT),
            Renderer::MULTISAMPLE_STATE.count,
        );

        Self { egui_renderer }
    }

    pub fn render(
        &mut self,
        ctx: &mut RenderContext,
        paint_jobs: &[egui::ClippedPrimitive],
        textures_delta: &egui::TexturesDelta,
    ) {
        for (texture_id, image_delta) in &textures_delta.set {
            self.egui_renderer
                .update_texture(ctx.device, ctx.queue, *texture_id, image_delta);
        }
        for texture_id in &textures_delta.free {
            self.egui_renderer.free_texture(texture_id);
        }

        let screen_descriptor = egui_wgpu::renderer::ScreenDescriptor {
            size_in_pixels: [ctx.surface_config.width, ctx.surface_config.height],
            pixels_per_point: 1.0,
        };

        self.egui_renderer.update_buffers(
            ctx.device,
            ctx.queue,
            &mut ctx.encoder,
            paint_jobs,
            &screen_descriptor,
        );

        self.egui_renderer.render(
            &mut ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: ctx.output.view,
                    resolve_target: ctx.output.resolve_target,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: ctx.output.depth_stencil,
                    depth_ops: None,
                    stencil_ops: None,
                }),
            }),
            paint_jobs,
            &screen_descriptor,
        );
    }

    pub fn engine_ui(engine: &mut Engine) -> impl FnOnce(&mut egui::Ui) + '_ {
        move |ui| {
            egui::CollapsingHeader::new("Adapter")
                .default_open(true)
                .show(ui, EguiPass::adapter_info_ui(&engine.renderer.adapter_info));

            egui::CollapsingHeader::new("Profiler")
                .default_open(true)
                .show(
                    ui,
                    EguiPass::profiler_ui(&engine.renderer.profiler_results()),
                );

            egui::CollapsingHeader::new("Gamma")
                .default_open(true)
                .show(ui, EguiPass::gamma_config_ui(&mut engine.config.gamma));

            egui::CollapsingHeader::new("Ambient")
                .default_open(true)
                .show(ui, EguiPass::ambient_config_ui(&mut engine.config.ambient));

            egui::CollapsingHeader::new("SSAO")
                .default_open(true)
                .show(ui, EguiPass::ssao_config_ui(&mut engine.config.ssao));
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

    pub fn profiler_ui(results: &[ProfilerResult]) -> impl FnOnce(&mut egui::Ui) + '_ {
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

    pub fn gamma_config_ui<'cfg: 'ui, 'ui>(
        gamma: &'cfg mut f32,
    ) -> impl FnOnce(&mut egui::Ui) + 'ui {
        move |ui| {
            ui.add(egui::Slider::new(gamma, 1.0..=3.0).text("Gamma"));
        }
    }

    pub fn ambient_config_ui<'cfg: 'ui, 'ui>(
        config: &'cfg mut AmbientConfig,
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
}
