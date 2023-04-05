#![warn(clippy::all)]

use crate::{RenderContext, Renderer};

pub struct EguiPass {
    pub context: egui::Context,

    paint_jobs: Vec<egui::ClippedPrimitive>,
    screen_descriptor: egui_wgpu::renderer::ScreenDescriptor,
    egui_renderer: egui_wgpu::Renderer,
}

impl EguiPass {
    pub fn new(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        scale_factor: f32,
    ) -> Self {
        let egui_renderer = egui_wgpu::Renderer::new(device, surface_config.format, None, 1);

        let screen_descriptor = egui_wgpu::renderer::ScreenDescriptor {
            size_in_pixels: [surface_config.width, surface_config.height],
            pixels_per_point: scale_factor,
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

    pub fn resize(&mut self, surface_config: &wgpu::SurfaceConfiguration, scale_factor: f32) {
        self.screen_descriptor = egui_wgpu::renderer::ScreenDescriptor {
            size_in_pixels: [surface_config.width, surface_config.height],
            pixels_per_point: scale_factor,
        };
    }

    pub fn update(
        &mut self,
        renderer: &Renderer,
        shapes: Vec<egui::epaint::ClippedShape>,
        textures_delta: egui::TexturesDelta,
    ) {
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
}

#[cfg(feature = "winit")]
pub use self::winit::*;
#[cfg(feature = "winit")]
mod winit {
    use winit::event_loop::EventLoop;

    use super::EguiPass;
    use crate::Renderer;

    pub struct EguiWinitPass {
        pass: EguiPass,
        state: egui_winit::State,
    }

    impl EguiWinitPass {
        pub fn new(
            device: &wgpu::Device,
            surface_config: &wgpu::SurfaceConfiguration,
            scale_factor: f32,
            event_loop: &EventLoop<()>,
        ) -> Self {
            Self {
                pass: EguiPass::new(device, surface_config, scale_factor),
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

    impl std::ops::Deref for EguiWinitPass {
        type Target = EguiPass;

        fn deref(&self) -> &Self::Target {
            &self.pass
        }
    }

    impl std::ops::DerefMut for EguiWinitPass {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.pass
        }
    }
}
