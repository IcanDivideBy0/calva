use anyhow::Result;
use calva::{
    egui::EguiPass,
    renderer::{
        wgpu, AmbientPass, DrawModel, GeometryBuffer, PointLight, PointLightsPass, Renderer,
        SsaoPass,
    },
};
use std::time::{Duration, Instant};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod camera;
mod debug_lights;
mod my_app;
mod shapes;

use camera::MyCamera;
use debug_lights::DebugLights;
use my_app::*;

struct Scene {
    models: Vec<Box<dyn DrawModel>>,
    lights: Vec<PointLight>,
    lights_vel: Vec<glam::Vec3>,
}

impl Scene {
    // const NUM_LIGHTS: usize = calva::renderer::PointLightsPass::MAX_LIGHTS;
    const NUM_LIGHTS: usize = 1000;

    pub fn new(renderer: &Renderer) -> Result<Self> {
        let get_random_vec3 = || glam::vec3(rand::random(), rand::random(), rand::random());

        let models: Vec<Box<dyn DrawModel>> = vec![
            // Box::new(shapes::SimpleMesh::new(
            //     &renderer,
            //     shapes::SimpleShape::Cube,
            //     "Cube",
            // )),
            Box::new(calva::gltf::loader::load(
                &renderer,
                &mut std::fs::File::open("./demo/assets/sponza.glb")?,
                // &mut std::fs::File::open("./demo/assets/zombie.glb")?,
                // &mut std::fs::File::open("./demo/assets/dungeon.glb")?,
                // &mut std::fs::File::open("./demo/assets/plane.glb")?,
            )?),
        ];

        let lights = (0..Self::NUM_LIGHTS)
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

        // let lights_vel = (0..Self::NUM_LIGHTS)
        //     .map(|_| glam::vec3(0.0, 0.0, 0.0))
        //     .collect::<Vec<_>>();

        Ok(Self {
            models,
            lights,
            lights_vel,
        })
    }

    pub fn update(&mut self, dt: Duration) {
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
        for (light, vel) in self.lights.iter_mut().zip(&self.lights_vel) {
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
    }
}

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

    let mut gbuffer = GeometryBuffer::new(&renderer);
    let mut ssao = SsaoPass::new(&renderer, &gbuffer.normal_roughness, &gbuffer.depth);
    let mut ambient = AmbientPass::new(&renderer, &gbuffer.albedo_metallic, &ssao.output);
    let mut point_lights = PointLightsPass::new(
        &renderer,
        &gbuffer.albedo_metallic,
        &gbuffer.normal_roughness,
        &gbuffer.depth,
        &ssao.output,
    );
    let mut debug_lights = DebugLights::new(&renderer);

    let mut my_app: MyApp = renderer.config.data.into();
    let mut egui = EguiPass::new(&renderer, &window);

    let mut scene = Scene::new(&renderer)?;

    // let start_time = Instant::now();
    let mut last_render_time = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        macro_rules! handle_resize {
            ($size: expr) => {{
                renderer.resize($size);

                gbuffer = GeometryBuffer::new(&renderer);
                ssao = SsaoPass::new(&renderer, &gbuffer.normal_roughness, &gbuffer.depth);
                ambient = AmbientPass::new(&renderer, &gbuffer.albedo_metallic, &ssao.output);
                point_lights = PointLightsPass::new(
                    &renderer,
                    &gbuffer.albedo_metallic,
                    &gbuffer.normal_roughness,
                    &gbuffer.depth,
                    &ssao.output,
                );
                debug_lights = DebugLights::new(&renderer);

                camera.resize($size);
            }};
        }

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

                scene.update(dt);

                match renderer.render(|ctx| {
                    gbuffer.render(ctx, &scene.models);
                    ssao.render(ctx);
                    ambient.render(ctx);
                    point_lights.render(ctx, &scene.lights);
                    debug_lights.render(ctx, &scene.lights);
                    egui.render(ctx, &window, &mut my_app).unwrap();
                }) {
                    Ok(_) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => handle_resize!(winit::dpi::PhysicalSize::new(
                        renderer.surface_config.width,
                        renderer.surface_config.height,
                    )),
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

                    WindowEvent::Resized(physical_size) => handle_resize!(*physical_size),
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        handle_resize!(**new_inner_size)
                    }

                    _ => {}
                }
            }

            _ => {}
        }
    });
}
