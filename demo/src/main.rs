#![warn(clippy::all)]

use anyhow::Result;
use async_std::task;
use calva::renderer::{
    wgpu, AmbientLightConfig, Camera, EguiWinitPass, Engine, Renderer, SkyboxManager,
};
use core::f32;
use glam::Vec3Swizzles;
use rand::seq::IteratorRandom;

use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    window::{Fullscreen, Window, WindowId},
};

pub mod camera;
pub mod controls;
pub mod debug;
pub mod monsters;
pub mod worldgen;

use crate::{
    camera::PerspectiveCamera, controls::TopDownCamera, monsters::MonstersManager,
    worldgen::WorldGenerator,
};

struct DemoState {
    window: Arc<Window>,
    engine: Engine,

    egui: EguiWinitPass,
    mouse_pos: glam::Vec2,
    kb_modifiers: ModifiersState,
}

impl DemoState {
    pub fn new(window: Arc<Window>, engine: Engine) -> Self {
        let _ = engine.resources.read::<PerspectiveCamera>();
        let _ = engine.resources.read::<WorldGenerator>();
        let _ = engine.resources.read::<TopDownCamera>();

        // *engine.resources.write::<controls::FlyingCamera>() = controls::FlyingCamera::from_look_at(
        //     glam::Vec3::Y + glam::Vec3::Z * 0.0, // eye
        //     glam::Vec3::Y - glam::Vec3::Z,       // target
        //     glam::Vec3::Y,                       // up
        // );

        *engine.resources.write::<AmbientLightConfig>() = AmbientLightConfig {
            color: [0.106535, 0.061572, 0.037324],
            strength: 0.1,
        };

        let egui = EguiWinitPass::new(&engine.resources, &window);

        Self::init_skybox(&engine).unwrap();

        Self {
            window,
            engine,

            egui,
            mouse_pos: Default::default(),
            kb_modifiers: Default::default(),
        }
    }

    fn init_skybox(engine: &Engine) -> Result<()> {
        let pixels = [
            "./demo/assets/sky/right.jpg",
            "./demo/assets/sky/left.jpg",
            "./demo/assets/sky/top.jpg",
            "./demo/assets/sky/bottom.jpg",
            "./demo/assets/sky/front.jpg",
            "./demo/assets/sky/back.jpg",
        ]
        .iter()
        .try_fold(vec![], |mut bytes, filepath| {
            let image = image::open(filepath)?;
            bytes.append(&mut image.to_rgba8().to_vec());
            Ok::<_, image::ImageError>(bytes)
        })?;

        engine
            .resources
            .write::<SkyboxManager>()
            .set_skybox(&pixels);

        Ok(())
    }
}

#[derive(Default)]
struct DemoApp {
    state: Option<DemoState>,
}

impl ApplicationHandler for DemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );

        let engine = Engine::new(
            task::block_on(Renderer::new(
                Box::new(event_loop.owned_display_handle()),
                window.clone(),
                window.inner_size().into(),
            ))
            .unwrap(),
        );

        self.state = Some(DemoState::new(window, engine));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        if state.egui.on_event(&state.window, &event).consumed {
            return;
        };

        // let mut flying_camera = state.engine.resources.write::<controls::FlyingCamera>();
        // if flying_camera.handle_event(&event) {
        //     // return;
        // }
        // drop(flying_camera);

        match event {
            WindowEvent::Resized(size) => {
                state.engine.resize(size.width, size.height);
            }

            WindowEvent::RedrawRequested => {
                state.egui.update(&state.window, |ui| {
                    ui.add(&mut *state.engine.resources.write::<MonstersManager>());
                    ui.add(&mut state.engine);
                });

                let result = state.engine.render(|ctx| {
                    state.egui.render(ctx);

                    Ok(())
                });

                if let Err(err) = result {
                    eprintln!("{err:?}");
                }

                // Emits a new redraw requested event.
                state.window.request_redraw();
            }

            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        ..
                    },
                ..
            } => event_loop.exit(),

            WindowEvent::ModifiersChanged(modifiers) => state.kb_modifiers = modifiers.state(),

            WindowEvent::CursorMoved { position, .. } => {
                state.mouse_pos = glam::Vec2::new(position.x as f32, position.y as f32);
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let camera = state.engine.resources.read::<Camera>();
                let worldgen = state.engine.resources.read::<WorldGenerator>();
                let surface_config = state.engine.resources.read::<wgpu::SurfaceConfiguration>();

                let (ro, rd) = camera.ray_cast(
                    state.mouse_pos,
                    glam::vec2(surface_config.width as f32, surface_config.height as f32),
                );

                if let Some(hit) = worldgen.ray_cast(ro, rd) {
                    let mut monsters = state.engine.resources.write::<MonstersManager>();

                    let hit = ro + rd * hit;

                    let mut rng = rand::rng();
                    let model = monsters.models.values().choose(&mut rng).unwrap();
                    let animation = model.animations.values().choose(&mut rng).unwrap();

                    let transform = glam::Mat4::from_translation(hit)
                        * glam::Mat4::from_axis_angle(
                            glam::Vec3::Y,
                            rand::random::<f32>() * f32::consts::TAU,
                        );

                    let object = model
                        .object()
                        .with_animation((*animation).into())
                        .with_transform(transform);

                    monsters.objects.push(object);
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Middle,
                ..
            } => {
                let camera = state.engine.resources.read::<Camera>();
                let worldgen = state.engine.resources.read::<WorldGenerator>();
                let surface_config = state.engine.resources.read::<wgpu::SurfaceConfiguration>();

                let (ro, rd) = camera.ray_cast(
                    state.mouse_pos,
                    glam::vec2(surface_config.width as f32, surface_config.height as f32),
                );

                if let Some(hit) = worldgen.ray_cast(ro, rd) {
                    let hit = ro + rd * hit;

                    state
                        .engine
                        .resources
                        .write::<MonstersManager>()
                        .set_target(hit.xz());
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                let camera = state.engine.resources.read::<Camera>();
                let worldgen = state.engine.resources.read::<WorldGenerator>();
                let surface_config = state.engine.resources.read::<wgpu::SurfaceConfiguration>();

                let (ro, rd) = camera.ray_cast(
                    state.mouse_pos,
                    glam::vec2(surface_config.width as f32, surface_config.height as f32),
                );

                if let Some(hit) = worldgen.ray_cast(ro, rd) {
                    let hit = ro + rd * hit;

                    let mut top_down_camera = state.engine.resources.write::<TopDownCamera>();
                    top_down_camera.target = hit;
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                match event {
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(KeyCode::KeyR),
                        ..
                    } => {
                        let mut monsters = state.engine.resources.write::<MonstersManager>();
                        monsters.objects.pop();
                    }

                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(KeyCode::KeyT),
                        ..
                    } => {
                        let mut monsters = state.engine.resources.write::<MonstersManager>();

                        let mut objects =
                            monsters
                                .models
                                .values()
                                .enumerate()
                                .flat_map(|(z, model)| {
                                    model.animations.values().enumerate().map(
                                        move |(x, animation)| {
                                            let transform = glam::Mat4::from_translation(
                                                glam::vec3(4.0 * x as f32, 8.0, 4.0 * z as f32),
                                            );

                                            model
                                                .object()
                                                .with_transform(transform)
                                                .with_animation((*animation).into())
                                        },
                                    )
                                })
                                .collect::<Vec<_>>();

                        monsters.objects.append(&mut objects);
                    }

                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(KeyCode::Enter),
                        ..
                    } if state.kb_modifiers.alt_key() => {
                        state
                            .window
                            .set_fullscreen(match state.window.fullscreen() {
                                None => Some(Fullscreen::Borderless(None)),
                                _ => None,
                            });
                    }

                    _ => {}
                }
            }
            _ => {}
        }
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new()?;

    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut DemoApp::default())?;

    Ok(())
}
