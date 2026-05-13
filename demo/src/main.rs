#![warn(clippy::all)]

use anyhow::Result;
use async_std::task;
use calva::{
    gltf::GltfModel,
    renderer::{
        wgpu, AmbientLightConfig, Camera, EguiWinitPass, Engine, Object, Renderer, SkyboxManager,
    },
};
use core::f32;

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
pub mod worldgen;

use crate::{camera::PerspectiveCamera, controls::FlyingCamera, worldgen::WorldGenerator};

struct DemoState {
    window: Arc<Window>,
    engine: Engine,

    egui: EguiWinitPass,
    mouse_pos: glam::Vec2,
    kb_modifiers: ModifiersState,

    monsters_models: Vec<GltfModel>,
    monster_objects: Vec<Object>,
}

impl DemoState {
    pub fn new(window: Arc<Window>, engine: Engine) -> Self {
        let _ = engine.resources.read::<PerspectiveCamera>();
        let _ = engine.resources.read::<WorldGenerator>();

        *engine.resources.write::<FlyingCamera>() = controls::FlyingCamera::from_look_at(
            glam::Vec3::Y + glam::Vec3::Z * 0.0, // eye
            glam::Vec3::Y - glam::Vec3::Z,       // target
            glam::Vec3::Y,                       // up
        );

        *engine.resources.write::<AmbientLightConfig>() = AmbientLightConfig {
            color: [0.106535, 0.061572, 0.037324],
            strength: 0.1,
        };

        let egui = EguiWinitPass::new(&engine.resources, &window);

        Self::init_skybox(&engine).unwrap();
        let monsters_models = Self::init_monsters(&engine).unwrap();

        Self {
            window,
            engine,

            egui,
            mouse_pos: Default::default(),
            kb_modifiers: Default::default(),

            monsters_models,
            monster_objects: Default::default(),
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

    fn init_monsters(engine: &Engine) -> Result<Vec<GltfModel>> {
        [
            "./demo/assets/zombies/zombie-boss.glb",
            "./demo/assets/zombies/zombie-common.glb",
            "./demo/assets/zombies/zombie-fat.glb",
            "./demo/assets/zombies/zombie-murderer.glb",
            "./demo/assets/zombies/zombie-snapper.glb",
            "./demo/assets/skeletons/skeleton-archer.glb",
            "./demo/assets/skeletons/skeleton-grunt.glb",
            "./demo/assets/skeletons/skeleton-mage.glb",
            "./demo/assets/skeletons/skeleton-king.glb",
            "./demo/assets/skeletons/skeleton-swordsman.glb",
            "./demo/assets/demons/demon-bomb.glb",
            "./demo/assets/demons/demon-boss.glb",
            "./demo/assets/demons/demon-fatty.glb",
            "./demo/assets/demons/demon-grunt.glb",
            "./demo/assets/demons/demon-imp.glb",
        ]
        .iter()
        // .take(1)
        .map(|filepath| GltfModel::from_path(&engine.resources, filepath))
        .collect::<Result<Vec<_>>>()
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

        if state
            .engine
            .resources
            .write::<FlyingCamera>()
            .handle_event(&event)
        {
            // return;
        }

        match event {
            WindowEvent::Resized(size) => {
                state.engine.resize(size.width, size.height);
            }

            WindowEvent::RedrawRequested => {
                // Update monster pos
                // if let (Some(height_map), Some(heat_map)) = (&state.height_map, &state.heat_map) {
                //     for monster in &mut state.monster_objects {
                //         let mut transform = monster.transform();
                //         let (_, _, translation) = transform.to_scale_rotation_translation();

                //         let grid_coord = Tile::get_grid_coord(&translation.xz());

                //         let dir = heat_map.apply_kernel(grid_coord) * Tile::PIXEL_SIZE / 4.0;

                //         let new_grid_coord = Tile::get_grid_coord(&(translation.xz() + dir));

                //         let dh = height_map.get_height(&new_grid_coord).unwrap_or_default()
                //             - height_map.get_height(&grid_coord).unwrap_or_default();

                //         transform =
                //             glam::Mat4::from_translation(glam::vec3(dir.x, dh, dir.y)) * transform;

                //         monster.set_transform(transform);
                //     }
                // }

                state.egui.update(&state.window, |ui| {
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
                // button,
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

                    let monster_model = &state.monsters_models
                        [rand::random::<u32>() as usize % state.monsters_models.len()];

                    let animation_keys = monster_model.animations.keys().collect::<Vec<_>>();
                    let animation = &monster_model.animations
                        [animation_keys[rand::random::<u32>() as usize % animation_keys.len()]];

                    let transform = glam::Mat4::from_translation(hit)
                        * glam::Mat4::from_axis_angle(
                            glam::Vec3::Y,
                            rand::random::<f32>() * f32::consts::TAU,
                        );

                    state.monster_objects.push(
                        monster_model
                            .object()
                            .with_animation((*animation).into())
                            .with_transform(transform),
                    );
                }

                // if let Some(height_map) = state.height_map.as_ref() {
                //     let camera = state.engine.resources.read::<Camera>();
                //     let surface_config =
                //         state.engine.resources.read::<wgpu::SurfaceConfiguration>();

                //     let (ro, rd) = camera.ray_cast(
                //         state.mouse_pos,
                //         glam::vec2(surface_config.width as f32, surface_config.height as f32),
                //     );

                //     if let Some(hit) = height_map.ray_cast(ro, rd) {
                //         if button == MouseButton::Left {
                //             let grid_coord = Tile::get_grid_coord(&hit.xz());
                //             let heat_map = dbg!(HeatMap::new(height_map, grid_coord));
                //             state.heat_map = Some(heat_map);
                //         }

                //         if button == MouseButton::Right {
                //             let transform = glam::Mat4::from_translation(hit)
                //                 * glam::Mat4::from_axis_angle(
                //                     glam::Vec3::Y,
                //                     rand::random::<f32>() * f32::consts::TAU,
                //                 );

                //             let monster_model = &state.monsters_models
                //                 [rand::random::<u32>() as usize % state.monsters_models.len()];

                //             let animation_keys =
                //                 monster_model.animations.keys().collect::<Vec<_>>();
                //             let animation = &monster_model.animations[animation_keys
                //                 [rand::random::<u32>() as usize % animation_keys.len()]];

                //             state.monster_objects.push(
                //                 monster_model
                //                     .object()
                //                     .with_transform(transform)
                //                     .with_animation((*animation).into()),
                //             );
                //         }
                //     }
                // }
            }

            WindowEvent::KeyboardInput { event, .. } => match event {
                KeyEvent {
                    state: ElementState::Pressed,
                    physical_key: PhysicalKey::Code(KeyCode::KeyR),
                    ..
                } => {
                    state.monster_objects.pop();
                }

                KeyEvent {
                    state: ElementState::Pressed,
                    physical_key: PhysicalKey::Code(KeyCode::KeyT),
                    ..
                } => {
                    for (z, monster_model) in state.monsters_models.iter().enumerate() {
                        for (x, animation) in monster_model.animations.values().enumerate() {
                            for y in 0..1 {
                                let transform = glam::Mat4::from_translation(glam::vec3(
                                    4.0 * x as f32,
                                    8.0 + 4.0 * y as f32,
                                    4.0 * z as f32,
                                ));

                                state.monster_objects.push(
                                    monster_model
                                        .object()
                                        .with_transform(transform)
                                        .with_animation((*animation).into()),
                                );
                            }
                        }
                    }
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
            },
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
