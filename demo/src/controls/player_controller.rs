use winit::event::{ElementState, WindowEvent};

use crate::worldgen::navgrid::NavGrid;

#[derive(Default)]
pub struct PlayerController {
    mouse_pos: glam::Vec2,
}

impl PlayerController {
    pub fn handle_event(
        &mut self,
        event: &WindowEvent,
        _navgrid: &Option<NavGrid>,
        cam_transform: glam::Mat4,
        cam_projection: glam::Mat4,
        viewport_size: glam::Vec2,
    ) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = glam::Vec2::new(position.x as f32, position.y as f32);
                false
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            } => {
                let (ro, rd) = {
                    let origin = cam_transform.col(3).truncate();

                    let mouse_pos_norm = glam::vec2(
                        (2.0 * self.mouse_pos.x) / viewport_size.x - 1.0,
                        1.0 - (2.0 * self.mouse_pos.y) / viewport_size.y,
                    );

                    let mut ray_eye = cam_projection.inverse()
                        * glam::vec4(mouse_pos_norm.x, mouse_pos_norm.y, -1.0, 1.0);
                    ray_eye.z = -1.0;
                    ray_eye.w = 0.0;

                    let direction = (cam_transform * ray_eye).truncate().normalize();

                    (origin, direction)
                };

                dbg!((ro, rd));

                // let transform = glam::Mat4::from_translation(origin);

                // let monster = self.monsters_models.first().unwrap();
                // let instances_handles = state
                //     .engine
                //     .resources
                //     .get::<InstancesManager>()
                //     .get_mut()
                //     .add(
                //         &monster
                //             .scene_instances(
                //                 None,
                //                 Some(transform),
                //                 monster.animations.get("run").copied(),
                //             )
                //             .unwrap()
                //             .0,
                //     );
                // self.monsters_instances.extend(instances_handles);

                true
            }

            _ => false,
        }
    }
}
