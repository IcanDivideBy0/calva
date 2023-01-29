#![warn(clippy::all)]

use anyhow::Result;
use calva::{
    egui::{egui, EguiPass},
    gltf::GltfModel,
    renderer::{wgpu, AmbientPass, GeometryPass, LightsPass, Renderer, SkyboxPass, SsaoPass},
};
use std::time::Instant;
use winit::{
    dpi::PhysicalSize,
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod app;
mod camera;

#[async_std::main]
async fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop)?;

    let mut camera = camera::MyCamera::new(&window);
    camera.controller.transform = glam::Mat4::look_at_rh(
        glam::Vec3::Y + glam::Vec3::Z * 12.0, // eye
        glam::Vec3::Y - glam::Vec3::Z,        // target
        glam::Vec3::Y,                        // up
    )
    .inverse();

    let mut renderer = Renderer::new(&window).await?;

    let mut geometry = GeometryPass::new(&renderer);
    let skybox = {
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

        SkyboxPass::new(&renderer, size, &bytes)
    };
    let mut ambient = AmbientPass::new(&renderer, geometry.albedo_metallic_view());
    let mut lights = LightsPass::new(
        &renderer,
        geometry.albedo_metallic_view(),
        geometry.normal_roughness_view(),
        &renderer.depth,
    );
    let mut ssao =
        SsaoPass::<800, 600>::new(&renderer, geometry.normal_roughness_view(), &renderer.depth);

    let egui_context = egui::Context::default();
    let mut egui_state = egui_winit::State::new(&event_loop);
    let mut egui = EguiPass::new(&renderer);
    let mut demo_app = app::DemoApp::default();

    let models = vec![
        // GltfModel::from_reader(
        //     &mut renderer,
        //     &mut geometry,
        //     &mut std::fs::File::open("./demo/assets/sphere.glb")?,
        // )?,
        // GltfModel::from_reader(
        //     &mut renderer,
        //     &mut geometry,
        //     &mut std::fs::File::open("./demo/assets/sponza.glb")?,
        // )?,
        // GltfModel::from_reader(
        //     &mut renderer,
        //     &mut geometry,
        //     &mut std::fs::File::open("./demo/assets/plane.glb")?,
        // )?,
        GltfModel::from_reader(
            &mut renderer,
            &mut geometry,
            &mut std::fs::File::open("./demo/assets/dungeon.glb")?,
        )?,
        GltfModel::from_reader(
            &mut renderer,
            &mut geometry,
            &mut std::fs::File::open("./demo/assets/zombie.glb")?,
        )?, // .map(|mut zombie| {
            //     let instance = zombie.instances[0];

            //     zombie.instances = (0..100)
            //         .map(|idx| {
            //             let mut i = instance;
            //             i.transform =
            //                 glam::Mat4::from_translation(glam::vec3(4.0 * idx as f32, 0.0, 0.0))
            //                     * i.transform;
            //             i
            //         })
            //         .collect();

            //     zombie
            // })?,
    ];

    // let objects = [
    //     // "./demo/assets/sphere.glb",
    //     "./demo/assets/sponza.glb",
    //     // "./demo/assets/plane.glb",
    //     "./demo/assets/zombie.glb",
    // ]
    // .iter()
    // .map(|path| {
    //     GltfModel::from_reader(
    //         &mut renderer,
    //         &mut geometry,
    //         &mut std::fs::File::open(path)?,
    //     )
    // })
    // .collect::<Result<Vec<_>>>()?;

    let mut instances = models
        .iter()
        .flat_map(|model| model.instances.iter().copied())
        .collect::<Vec<_>>();

    let point_lights = models
        .iter()
        .flat_map(|model| model.point_lights.iter().copied())
        .collect::<Vec<_>>();

    let mut last_render_time = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::RedrawRequested(_) => {
                let size = PhysicalSize::new(
                    renderer.surface_config.width,
                    renderer.surface_config.height,
                );
                let window_size = window.inner_size();
                if size != window_size {
                    camera.resize(window_size);
                    renderer.resize(window_size);
                    geometry.resize(&renderer);
                    ambient.resize(&renderer, geometry.albedo_metallic_view());
                    lights.resize(
                        &renderer,
                        geometry.albedo_metallic_view(),
                        geometry.normal_roughness_view(),
                        &renderer.depth,
                    );
                    ssao.resize(&renderer, geometry.normal_roughness_view(), &renderer.depth);
                }

                let dt = last_render_time.elapsed();
                last_render_time = Instant::now();

                camera.controller.update(dt);
                renderer.camera.update(
                    &renderer.queue,
                    camera.controller.transform.inverse(),
                    camera.projection.into(),
                );

                let (paint_jobs, textures_delta) = {
                    let output = egui_context.run(egui_state.take_egui_input(&window), |ctx| {
                        demo_app.ui(ctx, &mut renderer, &mut ambient.config, &mut ssao.config)
                    });

                    egui_state.handle_platform_output(
                        &window,
                        &egui_context,
                        output.platform_output,
                    );

                    (
                        egui_context.tessellate(output.shapes),
                        output.textures_delta,
                    )
                };

                for instance in instances.iter_mut() {
                    instance.animation.time += dt.as_secs_f32();
                }

                let result = renderer.render(|ctx| {
                    geometry.render(ctx, &instances);
                    ambient.render(ctx, demo_app.gamma);
                    lights.render(ctx, demo_app.gamma, &point_lights);
                    ssao.render(ctx);
                    skybox.render(ctx, demo_app.gamma);

                    egui.render(
                        ctx,
                        &paint_jobs,
                        &textures_delta,
                        1.0,
                        // window.scale_factor() as _,
                    );
                });

                match result {
                    Ok(_) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => renderer.resize(PhysicalSize::new(0, 0)),
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

            Event::WindowEvent { ref event, .. } => {
                if egui_state.on_event(&egui_context, event).consumed {
                    return;
                }

                if camera.handle_event(event) {
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
                    _ => {}
                }
            }

            _ => {}
        }
    });
}

// #![warn(clippy::all)]

// use anyhow::Result;
// use calva::{
//     egui::EguiPass,
//     renderer::{graph, wgpu, DirectionalLight, Renderer, RendererConfigData},
// };
// use std::time::Instant;
// use winit::{
//     event::*,
//     event_loop::{ControlFlow, EventLoop},
//     window::WindowBuilder,
// };

// mod camera;
// mod my_app;
// mod particle;

// use camera::MyCamera;
// use my_app::*;
// use particle::*;

// #[async_std::main]
// async fn main() -> Result<()> {
//     env_logger::init();
//     let event_loop = EventLoop::new();
//     let window = WindowBuilder::new().build(&event_loop)?;

//     let mut renderer = Renderer::new(&window).await?;

//     Ok(())
// }

// // #[async_std::main]
// async fn _main() -> Result<()> {
//     env_logger::init();
//     let event_loop = EventLoop::new();
//     let window = WindowBuilder::new().build(&event_loop)?;

//     let mut camera = MyCamera::new(&window);
//     camera.controller.transform = glam::Mat4::look_at_rh(
//         glam::Vec3::Y,                 // eye
//         glam::Vec3::Y + glam::Vec3::X, // target
//         glam::Vec3::Y,                 // up
//     )
//     .inverse();

//     let mut renderer = Renderer::new(&window).await?;

//     let skybox = {
//         let mut size = 0;
//         let mut bytes = vec![];

//         let images = [
//             image::open("./demo/assets/sky/right.jpg")?,
//             image::open("./demo/assets/sky/left.jpg")?,
//             image::open("./demo/assets/sky/top.jpg")?,
//             image::open("./demo/assets/sky/bottom.jpg")?,
//             image::open("./demo/assets/sky/front.jpg")?,
//             image::open("./demo/assets/sky/back.jpg")?,
//         ];

//         for image in images {
//             let image = image.to_rgba8();
//             size = image.width();
//             bytes.append(&mut image.to_vec());
//         }

//         (size, bytes)
//     };

//     let mut rgraph = graph::DefaultGraph::new(&renderer, (skybox.0, &skybox.1));
//     let mut egui = EguiPass::new(&renderer, &window);

//     let mut models = {
//         // let sponza = calva::gltf::GltfModel::new(
//         //     &renderer,
//         //     &mut geo,
//         //     &mut std::fs::File::open("./demo/assets/sponza.glb")?,
//         // )?;

//         let dungeon = calva::gltf::GltfModel::new(
//             &renderer,
//             &mut std::fs::File::open("./demo/assets/dungeon.glb")?,
//         )?;

//         // let plane = calva::gltf::GltfModel::new(
//         //     &renderer,
//         //     &mut geo,
//         //     &mut std::fs::File::open("./demo/assets/plane.glb")?,
//         // )?;

//         let mut zombie = calva::gltf::GltfModel::new(
//             &renderer,
//             &mut std::fs::File::open("./demo/assets/zombie.glb")?,
//         )?;

//         for (mesh_instances, skin_animation_instances) in zombie.instances.iter_mut() {
//             if let Some(mesh_instance) = mesh_instances.get(0) {
//                 **mesh_instances = zombie.animations[0]
//                     .animations
//                     .iter()
//                     .enumerate()
//                     .map(|(i, _)| {
//                         let transform =
//                             glam::Mat4::from_translation(glam::Vec3::X * 3.0 * i as f32)
//                                 * glam::Mat4::from(mesh_instance);

//                         (&transform).into()
//                     })
//                     .collect();

//                 if let Some(skin_animation_instances) = skin_animation_instances {
//                     **skin_animation_instances = zombie.animations[0]
//                         .animations
//                         .iter()
//                         .map(|(_, (offset, _))| calva::renderer::SkinAnimationInstance {
//                             frame: *offset,
//                         })
//                         .collect();
//                 }
//             }
//         }

//         // vec![dungeon]
//         // vec![sponza, zombie]
//         vec![dungeon, zombie]
//         // vec![plane]
//     };

//     for model in &mut models {
//         for (mesh_instances, skin_animation_instances) in model.instances.iter_mut() {
//             mesh_instances.write_buffer(&renderer.queue);

//             if let Some(skin_animation_instances) = skin_animation_instances {
//                 skin_animation_instances.write_buffer(&renderer.queue);
//             }
//         }
//     }

//     let point_lights = models.iter().fold(vec![], |mut acc, model| {
//         acc.extend(&model.point_lights);
//         acc
//     });

//     let particles = Particles::new(
//         &renderer.device,
//         &models[1].instances[0].0,
//         models[1].instances[0].1.as_ref().unwrap(),
//     );

//     let mut my_app: MyApp = renderer.config.data.into();
//     let mut last_render_time = Instant::now();

//     event_loop.run(move |event, _, control_flow| {
//         macro_rules! handle_resize {
//             ($size: expr) => {{
//                 camera.resize($size);
//                 renderer.resize($size);
//                 rgraph = graph::DefaultGraph::new(&renderer, (skybox.0, &skybox.1));
//             }};
//         }

//         egui.handle_event(&event);
//         if egui.captures_event(&event) {
//             return;
//         }

//         match event {
//             Event::RedrawRequested(_) => {
//                 let dt = last_render_time.elapsed();
//                 last_render_time = Instant::now();

//                 renderer.config.data = RendererConfigData::from(&my_app);
//                 camera.update(&mut renderer, dt);

//                 match renderer.render(|ctx| {
//                     geo.render(&mut ctx.encoder, &renderer.camera, |draw| {});

//                     particles.run(ctx, &models[1].animations[0]);

//                     rgraph.render(
//                         ctx,
//                         |draw| {
//                             for model in &models {
//                                 for (mesh, skin, material_index, instances_index) in &model.meshes {
//                                     let instances = &model.instances[*instances_index];
//                                     let material = &model.materials[*material_index];

//                                     draw((
//                                         &instances.0,
//                                         mesh,
//                                         material,
//                                         skin.as_ref(),
//                                         instances.1.as_ref(),
//                                         model.animations.get(0),
//                                     ));
//                                 }
//                             }
//                         },
//                         |draw| {
//                             for model in &models {
//                                 for (mesh, skin, _, instances_index) in &model.meshes {
//                                     let instances = &model.instances[*instances_index];

//                                     draw((
//                                         &instances.0,
//                                         mesh,
//                                         skin.as_ref(),
//                                         instances.1.as_ref(),
//                                         model.animations.get(0),
//                                     ));
//                                 }
//                             }
//                         },
//                         &DirectionalLight {
//                             direction: my_app.shadow_light_angle,
//                             color: glam::Vec4::ONE,
//                         },
//                         [5.0, 25.0, 64.0, 0.0],
//                         &point_lights,
//                     );

//                     // debug_lights.render(ctx, &point_lights);
//                     egui.render(ctx, &window, &mut my_app).unwrap();
//                 }) {
//                     Ok(_) => {}
//                     // Reconfigure the surface if lost
//                     Err(wgpu::SurfaceError::Lost) => handle_resize!(winit::dpi::PhysicalSize::new(
//                         renderer.surface_config.width,
//                         renderer.surface_config.height,
//                     )),
//                     // The system is out of memory, we should probably quit
//                     Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
//                     // All other errors (Outdated, Timeout) should be resolved by the next frame
//                     Err(e) => eprintln!("{:?}", e),
//                 }
//             }

//             Event::MainEventsCleared => {
//                 // RedrawRequested will only trigger once, unless we manually request it.
//                 window.request_redraw();
//             }

//             Event::WindowEvent {
//                 ref event,
//                 window_id,
//             } if window_id == window.id() => {
//                 if camera.process_event(event) {
//                     return;
//                 }

//                 match event {
//                     WindowEvent::CloseRequested
//                     | WindowEvent::KeyboardInput {
//                         input:
//                             KeyboardInput {
//                                 state: ElementState::Pressed,
//                                 virtual_keycode: Some(VirtualKeyCode::Escape),
//                                 ..
//                             },
//                         ..
//                     } => *control_flow = ControlFlow::Exit,

//                     WindowEvent::Resized(physical_size) => handle_resize!(*physical_size),
//                     WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
//                         handle_resize!(**new_inner_size)
//                     }

//                     _ => {}
//                 }
//             }

//             _ => {}
//         }
//     });
// }
