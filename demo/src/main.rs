use anyhow::Result;
use calva::{
    egui::EguiPass,
    renderer::{rpass, wgpu, PointLight, Renderer, RendererConfigData},
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
// mod shapes;

use camera::MyCamera;
use debug_lights::DebugLights;
use my_app::*;

struct Scene {
    // models: Vec<Box<dyn DrawModel>>,
    lights: Vec<PointLight>,
    lights_vel: Vec<glam::Vec3>,
}

impl Scene {
    // const NUM_LIGHTS: usize = calva::renderer::PointLightsPass::MAX_LIGHTS;
    const NUM_LIGHTS: usize = 1;

    pub fn new(_renderer: &Renderer) -> Result<Self> {
        let get_random_vec3 = || glam::vec3(rand::random(), rand::random(), rand::random());

        // let models: Vec<Box<dyn DrawModel>> = vec![
        //     Box::new(shapes::SimpleMesh::new(
        //         renderer,
        //         shapes::SimpleShape::Cube,
        //         "Cube",
        //         glam::Mat4::from_scale_rotation_translation(
        //             glam::Vec3::ONE,
        //             glam::Quat::IDENTITY,
        //             100_000.0 * glam::vec3(-1.0, 1.0, 0.0) + glam::Vec3::Y * 2.0,
        //         ),
        //         glam::vec3(0.0, 0.0, 1.0),
        //     )),
        //     Box::new(calva::gltf::loader::load(
        //         renderer,
        //         &mut std::fs::File::open("./demo/assets/model.glb")?,
        //         // &mut std::fs::File::open("./demo/assets/zombie.glb")?,
        //         // &mut std::fs::File::open("./demo/assets/dungeon.glb")?,
        //         // &mut std::fs::File::open("./demo/assets/plane.glb")?,
        //     )?),
        // ];

        let lights = (0..Self::NUM_LIGHTS)
            .map(|_| PointLight {
                // position: (get_random_vec3() * 2.0 - 1.0) * 5.0,
                position: glam::vec3(0.0, 0.0, 1.0),
                radius: 12.0,
                // color: get_random_vec3(),
                color: glam::Vec3::ONE,
            })
            .collect::<Vec<_>>();

        let _lights_vel = (0..lights.len())
            .map(|_| (get_random_vec3() * 2.0 - 1.0) * 2.0 * glam::vec3(0.0, 1.0, 0.0))
            .collect::<Vec<_>>();

        let lights_vel = (0..Self::NUM_LIGHTS)
            .map(|_| glam::vec3(0.0, 2.0, 0.0))
            .collect::<Vec<_>>();

        Ok(Self {
            // models,
            lights,
            lights_vel,
        })
    }

    pub fn update(&mut self, _t: Duration, dt: Duration) {
        // for (light, idx) in lights.iter_mut().zip(0..) {
        //     light.position = glam::vec3(
        //         (start_time.elapsed().as_secs_f32() + (idx as f32 / num_lights)).sin()
        //             * 1.0,
        //         2.0,
        //         (start_time.elapsed().as_secs_f32() + (idx as f32 / num_lights)).cos()
        //             * 1.0,
        //     );
        // }

        let limit = 5.0;
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

        // for mesh in self.models[0].meshes_mut() {
        //     let instances = mesh.instances_mut();

        //     instances.transforms[0] = glam::Mat4::from_scale_rotation_translation(
        //         glam::Vec3::ONE,
        //         glam::Quat::from_euler(glam::EulerRot::XYZ, t.as_secs_f32(), 0.0, 0.0),
        //         glam::Vec3::Y * 2.0,
        //     );
        // }

        // let cube: Box<&mut dyn std::any::Any> = Box::new(&mut self.models[0].as_mut());
        // if let Some(Model::Simple(m)) = cube.downcast_mut::<Model>() {
        //     m.instances.transforms[0] = glam::Mat4::from_scale_rotation_translation(
        //         glam::Vec3::ONE,
        //         glam::Quat::from_euler(glam::EulerRot::XYZ, dt.as_secs_f32(), 0.0, 0.0),
        //         glam::Vec3::Y * 2.0,
        //     );
        // } else {
        //     dbg!("nocube");
        // }
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop)?;

    let mut camera = MyCamera::new(&window);
    camera.controller.transform = glam::Mat4::look_at_rh(
        glam::Vec3::Y,                 // eye
        glam::Vec3::Y + glam::Vec3::X, // target
        glam::Vec3::Y,                 // up
    )
    .inverse();

    let mut renderer = Renderer::new(&window).await?;

    let skybox_data = {
        let mut size = 0;
        let mut bytes = vec![];

        let images = [
            image::open("./demo/assets/sky/right.jpg")?,
            image::open("./demo/assets/sky/left.jpg")?,
            image::open("./demo/assets/sky/top.jpg")?,
            image::open("./demo/assets/sky/bottom.jpg")?,
            image::open("./demo/assets/sky/front.jpg")?,
            image::open("./demo/assets/sky/back.jpg")?,
        ];

        for image in images {
            let image = image.to_rgba8();
            size = image.width();
            bytes.append(&mut image.to_vec());
        }

        (size, bytes)
    };

    let mut gbuffer = rpass::Geometry::new(&renderer);
    let mut skybox = rpass::Skybox::new(&renderer, skybox_data.0, &skybox_data.1);
    let mut ambient = rpass::Ambient::new(&renderer, &gbuffer.albedo_metallic);
    let mut shadows = rpass::ShadowLight::new(
        &renderer,
        &gbuffer.albedo_metallic,
        &gbuffer.normal_roughness,
        &gbuffer.depth,
    );
    let mut point_lights = rpass::PointLights::new(
        &renderer,
        &gbuffer.albedo_metallic,
        &gbuffer.normal_roughness,
        &gbuffer.depth,
    );
    let mut debug_lights = DebugLights::new(&renderer);
    let mut ssao = rpass::Ssao::new(&renderer, &gbuffer.normal_roughness, &gbuffer.depth);

    let mut my_app: MyApp = renderer.config.data.into();
    let mut egui = EguiPass::new(&renderer, &window);

    let mut scene = Scene::new(&renderer)?;
    let mut plane = calva::gltf::GltfModel::new(
        &renderer,
        &mut std::fs::File::open("./demo/assets/plane.glb")?,
    )?;

    let mut zombie = calva::gltf::GltfModel::new(
        &renderer,
        &mut std::fs::File::open("./demo/assets/zombie.glb")?,
    )?;

    for (mesh_instances, skin_animation_instances) in zombie.instances.iter_mut() {
        if let Some(mesh_instance) = mesh_instances.get(0) {
            **mesh_instances = zombie.animations[0]
                .animations
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let transform = glam::Mat4::from_translation(glam::Vec3::X * 3.0 * i as f32)
                        * glam::Mat4::from(mesh_instance);

                    (&transform).into()
                })
                .collect();

            if let Some(skin_animation_instances) = skin_animation_instances {
                **skin_animation_instances = zombie.animations[0]
                    .animations
                    .iter()
                    .map(|_| calva::renderer::SkinAnimationInstance { frame: 0 })
                    .collect();
            }
        }
    }

    my_app.animations = zombie.animations[0].animations.keys().cloned().collect();

    let start_time = Instant::now();
    let mut last_render_time = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        macro_rules! handle_resize {
            ($size: expr) => {{
                renderer.resize($size);

                gbuffer = rpass::Geometry::new(&renderer);
                skybox = rpass::Skybox::new(&renderer, skybox_data.0, &skybox_data.1);
                ambient = rpass::Ambient::new(&renderer, &gbuffer.albedo_metallic);
                shadows = rpass::ShadowLight::new(
                    &renderer,
                    &gbuffer.albedo_metallic,
                    &gbuffer.normal_roughness,
                    &gbuffer.depth,
                );
                point_lights = rpass::PointLights::new(
                    &renderer,
                    &gbuffer.albedo_metallic,
                    &gbuffer.normal_roughness,
                    &gbuffer.depth,
                );
                debug_lights = DebugLights::new(&renderer);
                ssao = rpass::Ssao::new(&renderer, &gbuffer.normal_roughness, &gbuffer.depth);

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

                renderer.config.data = RendererConfigData::from(&my_app);
                camera.update(&mut renderer, dt);
                scene.update(start_time.elapsed(), dt);
                scene.lights[0].position = my_app.light_pos;

                for (mesh_instances, _) in &mut plane.instances {
                    mesh_instances.write_buffer(&renderer.queue);
                }

                for (mesh_instances, skin_animation_instances) in zombie.instances.iter_mut() {
                    mesh_instances.write_buffer(&renderer.queue);

                    if let Some(skin_animation_instances) = skin_animation_instances {
                        for (i, skin_animation_instance) in
                            skin_animation_instances.iter_mut().enumerate()
                        {
                            let anim_name = my_app.animations[i].clone();

                            let (offset, length) = zombie.animations[0].animations[&anim_name];
                            let current_frame =
                                skin_animation_instance.frame.saturating_sub(offset);

                            skin_animation_instance.frame = offset + (current_frame + 1) % length;
                        }
                        skin_animation_instances.write_buffer(&renderer.queue);
                    }
                }

                match renderer.render(|ctx| {
                    gbuffer.render(ctx, |draw| {
                        for (mesh, skin, material_index, instances_index) in &plane.meshes {
                            let instances = plane.instances.get(*instances_index).unwrap();
                            let material = plane.materials.get(*material_index).unwrap();

                            draw((
                                &instances.0,
                                mesh,
                                material,
                                skin.as_ref(),
                                instances.1.as_ref(),
                                plane.animations.get(0),
                            ));
                        }

                        for (mesh, skin, material_index, instances_index) in &zombie.meshes {
                            let instances = zombie.instances.get(*instances_index).unwrap();
                            let material = zombie.materials.get(*material_index).unwrap();

                            draw((
                                &instances.0,
                                mesh,
                                material,
                                skin.as_ref(),
                                instances.1.as_ref(),
                                zombie.animations.get(0),
                            ));
                        }
                    });

                    skybox.render(ctx);
                    ambient.render(ctx);
                    shadows.render(ctx, my_app.shadow_light_angle, |draw| {
                        for (mesh, skin, _, instances_index) in &plane.meshes {
                            let instances = plane.instances.get(*instances_index).unwrap();
                            draw((
                                &instances.0,
                                mesh,
                                skin.as_ref(),
                                instances.1.as_ref(),
                                plane.animations.get(0),
                            ));
                        }
                        for (mesh, skin, _, instances_index) in &zombie.meshes {
                            let instances = zombie.instances.get(*instances_index).unwrap();
                            draw((
                                &instances.0,
                                mesh,
                                skin.as_ref(),
                                instances.1.as_ref(),
                                zombie.animations.get(0),
                            ));
                        }
                    });

                    point_lights.render(ctx, &scene.lights);
                    ssao.render(ctx);

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
