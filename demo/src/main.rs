use anyhow::Result;
use calva::renderer::{DrawModel, PointLight, Renderer};
use std::time::Instant;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod camera;
mod debug_lights;
mod egui;
mod my_app;
mod shapes;

use camera::MyCamera;
use debug_lights::DebugLights;
use my_app::*;

#[async_std::main]
async fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop)?;

    let mut camera = MyCamera::new(&window);
    camera.controller.transform = glam::Mat4::inverse(&glam::Mat4::look_at_rh(
        glam::Vec3::Y + glam::Vec3::ZERO, // eye
        glam::Vec3::Y + glam::Vec3::X,    // target
        glam::Vec3::Y,                    // up
    ));

    let mut renderer = Renderer::new(&window).await?;
    let mut debug_lights =
        DebugLights::new(&renderer.device, &renderer.surface_config, &renderer.camera);

    let mut egui = crate::egui::EguiPass::new(&window, &renderer.device);
    let mut my_app: MyApp = renderer.config.data.into();

    let models: Vec<Box<dyn DrawModel>> = vec![
        // Box::new(shapes::SimpleMesh::new(
        //     &renderer,
        //     shapes::SimpleShape::Cube,
        //     "Cube",
        // )),
        Box::new(calva::gltf::loader::load(
            &renderer,
            &mut std::fs::File::open("./assets/sponza.glb")?,
            // &mut std::fs::File::open("./assets/zombie.glb")?,
            // &mut std::fs::File::open("./assets/dungeon.glb")?,
            // &mut std::fs::File::open("./assets/plane.glb")?,
        )?),
    ];

    let get_random_vec3 = || glam::vec3(rand::random(), rand::random(), rand::random());

    let num_lights = 10; // calva::renderer::PointLightsPass::MAX_LIGHTS;
    let mut lights = (0..num_lights)
        .map(|_| PointLight {
            position: (get_random_vec3() * 2.0 - 1.0) * 15.0,
            radius: 1.0,
            color: get_random_vec3(),
            // color: glam::Vec3::ONE,
        })
        .collect::<Vec<_>>();

    let lights_vel = (0..lights.len())
        .map(|_| (get_random_vec3() * 2.0 - 1.0) * 2.0 * glam::vec3(0.0, 1.0, 0.0))
        .collect::<Vec<_>>();

    // let num_lights = 1;
    // let mut lights = (0..num_lights)
    //     .map(|_| PointLight {
    //         // position: 5.0 * glam::Vec3::X + glam::Vec3::Y * 0.1,
    //         position: glam::Vec3::Y,
    //         radius: 5.0,
    //         color: glam::Vec3::ONE,
    //     })
    //     .collect::<Vec<_>>();
    // let lights_vel = (0..lights.len())
    //     .map(|_| glam::vec3(0.0, 0.0, 0.0))
    //     .collect::<Vec<_>>();

    // let start_time = Instant::now();
    let mut last_render_time = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        egui.handle_event(&event);

        if egui.captures_event(&event) {
            return;
        }

        match event {
            Event::RedrawRequested(_) => {
                let dt = last_render_time.elapsed();
                last_render_time = Instant::now();

                renderer.config.data = my_app.into();
                camera.update(&mut renderer, dt);

                // lights[0].position = my_app.light_pos;

                // for (light, idx) in lights.iter_mut().zip(0..) {
                //     light.position = glam::vec3(
                //         (start_time.elapsed().as_secs_f32() + (idx as f32 / num_lights)).sin()
                //             * 1.0,
                //         2.0,
                //         (start_time.elapsed().as_secs_f32() + (idx as f32 / num_lights)).cos()
                //             * 1.0,
                //     );
                // }

                let limit = 15.0;
                for (light, vel) in lights.iter_mut().zip(&lights_vel) {
                    light.position += *vel * dt.as_secs_f32();

                    if light.position.x > limit {
                        light.position.x = -limit;
                    }
                    if light.position.x < -limit {
                        light.position.x = limit;
                    }

                    if light.position.y > limit {
                        light.position.y = -limit;
                    }
                    if light.position.y < -limit {
                        light.position.y = limit;
                    }

                    if light.position.z > limit {
                        light.position.z = -limit;
                    }
                    if light.position.z < -limit {
                        light.position.z = limit;
                    }
                }

                match renderer.begin_render_frame() {
                    Ok(mut ctx) => {
                        renderer.gbuffer.render(&mut ctx, &models);

                        renderer.ssao.render(&mut ctx);
                        renderer.ambient.render(&mut ctx);
                        renderer.lights.render(&mut ctx, &lights);

                        debug_lights.render(&mut ctx, &lights);

                        egui.render(&window, &mut ctx, &mut my_app).expect("egui");

                        renderer.finish_render_frame(ctx);
                    }
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => {
                        renderer.resize(winit::dpi::PhysicalSize::new(
                            renderer.surface_config.width,
                            renderer.surface_config.height,
                        ));
                        debug_lights = DebugLights::new(
                            &renderer.device,
                            &renderer.surface_config,
                            &renderer.camera,
                        );
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
                        camera.resize(*physical_size);
                        debug_lights = DebugLights::new(
                            &renderer.device,
                            &renderer.surface_config,
                            &renderer.camera,
                        );
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        renderer.resize(**new_inner_size);
                        camera.resize(**new_inner_size);
                        debug_lights = DebugLights::new(
                            &renderer.device,
                            &renderer.surface_config,
                            &renderer.camera,
                        );
                    }

                    _ => {}
                }
            }

            _ => {}
        }
    });
}
