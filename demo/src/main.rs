use anyhow::Result;
use calva::renderer::{
    AmbientPass, DrawModel, EguiPass, GeometryBuffer, LightsPass, PointLight, Renderer, SsaoPass,
};
use std::time::Instant;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod camera;
mod my_app;
mod shapes;

use camera::MyCamera;
use my_app::*;

#[async_std::main]
async fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop)?;
    // window.set_outer_position(winit::dpi::PhysicalPosition::new(1920 * 2, 0));

    let mut camera = MyCamera::new(&window);
    camera.controller.transform = glam::Mat4::inverse(&glam::Mat4::look_at_rh(
        (2.0, 2.0, 2.0).into(), // eye
        (0.0, 0.0, 0.0).into(), // target
        (0.0, 1.0, 0.0).into(), // up
    ));

    let mut renderer = Renderer::new(&window).await?;
    let mut gbuffer = GeometryBuffer::new(&renderer);
    let ssao = SsaoPass::new(&renderer, &gbuffer);
    let ambient = AmbientPass::new(&renderer, &gbuffer);
    let lights = LightsPass::new(&renderer, &gbuffer);

    let start_time = Instant::now();
    let mut last_render_time = Instant::now();

    let models: Vec<Box<dyn DrawModel>> = vec![
        // Box::new(shapes::SimpleMesh::new(
        //     &renderer,
        //     shapes::SimpleShape::Cube,
        //     "Plane",
        // )),
        Box::new(calva::gltf::loader::load(
            &renderer,
            &mut std::fs::File::open("./assets/zombie.glb")?,
        )?),
        // Box::new(calva::gltf::loader::load(
        //     &renderer,
        //     &mut std::fs::File::open("./assets/plane.glb")?,
        // )?),
    ];

    let mut egui = EguiPass::new(&window, &renderer.device);
    let mut my_app = MyApp::default();

    event_loop.run(move |event, _, control_flow| {
        egui.handle_event(&event);

        if egui.captures_event(&event) {
            return;
        }

        match event {
            Event::RedrawRequested(_) => {
                let now = std::time::Instant::now();
                let dt = now - last_render_time;
                last_render_time = now;

                camera.update(&mut renderer, dt);

                let t = Instant::now() - start_time;
                let lights_data = [
                    PointLight {
                        position: (
                            1.0,
                            3.0 + t.as_secs_f32().cos() / 2.0,
                            1.0 + t.as_secs_f32().sin() / 2.0,
                        )
                            .into(),
                        radius: 1.0,
                        color: (1.0, 0.0, 0.0).into(),
                    },
                    PointLight {
                        position: (
                            1.0,
                            2.0 + t.as_secs_f32().sin() / 2.0,
                            1.0 + t.as_secs_f32().cos() / 2.0,
                        )
                            .into(),
                        radius: 1.0,
                        color: (0.0, 1.0, 0.0).into(),
                    },
                ];

                match renderer.begin_render_frame() {
                    Ok(mut ctx) => {
                        gbuffer.render(&mut ctx, &models);
                        ssao.render(&mut ctx);
                        ambient.render(&mut ctx, &gbuffer);
                        lights.render(&mut ctx, &gbuffer, &lights_data);

                        egui.render(&window, &mut ctx, &mut my_app).expect("egui");

                        renderer.finish_render_frame(ctx);
                    }
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => {
                        renderer.resize(winit::dpi::PhysicalSize::new(
                            renderer.surface_config.width,
                            renderer.surface_config.height,
                        ));
                        gbuffer = GeometryBuffer::new(&renderer);
                    }
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
            }

            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually request it.
                window.request_redraw();
            }

            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                if camera.process_event(event) {
                    return;
                }

                match event {
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => *control_flow = ControlFlow::Exit,

                    WindowEvent::Resized(physical_size) => {
                        renderer.resize(*physical_size);
                        gbuffer = GeometryBuffer::new(&renderer);
                        camera.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        renderer.resize(**new_inner_size);
                        gbuffer = GeometryBuffer::new(&renderer);
                        camera.resize(**new_inner_size);
                    }

                    _ => {}
                }
            }

            _ => {}
        }
    });
}
