use anyhow::Result;
use calva::prelude::*;
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
        (10.0, 10.0, 10.0).into(), // eye
        (0.0, 0.0, 0.0).into(),    // target
        (0.0, 1.0, 0.0).into(),    // up
    ));

    let mut renderer = Renderer::new(&window).await?;
    let mut last_render_time = Instant::now();

    let mut file = std::fs::File::open("./assets/zombie.glb")?;
    let zombie = calva::gltf::loader::load(&mut file, &renderer)?;

    // let mut file = std::fs::File::open("./assets/dungeon.glb")?;
    // let dungeon = calva::gltf::loader::load(&mut file, &renderer)?;

    // let plane = shapes::plane::build_model(&renderer, "Plane");

    let models = vec![
        // plane,
        zombie,
        // dungeon
    ];

    let mut my_app = MyApp::default();

    event_loop.run(move |event, _, control_flow| {
        renderer.egui.handle_event(&event);

        if renderer.egui.captures_event(&event) {
            return;
        }

        match event {
            Event::RedrawRequested(_) => {
                let now = std::time::Instant::now();
                let dt = now - last_render_time;
                last_render_time = now;

                camera.update(dt);
                renderer.update_camera(&camera);

                match renderer.render(&window, &models, &mut my_app) {
                    Ok(_) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => {
                        renderer.resize(winit::dpi::PhysicalSize::new(
                            renderer.surface_config.width,
                            renderer.surface_config.height,
                        ))
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
                        camera.resize(*physical_size)
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        renderer.resize(**new_inner_size);
                        camera.resize(**new_inner_size);
                    }

                    _ => {}
                }
            }

            _ => {}
        }
    });
}
