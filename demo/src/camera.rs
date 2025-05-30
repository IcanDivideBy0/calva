use std::time::Duration;

use calva::renderer::Camera;
use winit::event::WindowEvent;

pub struct MyCamera {
    pub aspect: f32,
    pub fovy: f32, // rad
    pub znear: f32,
    pub zfar: f32,

    pub controller: FlyingCamera,
}

impl MyCamera {
    pub fn new((width, height): (u32, u32)) -> Self {
        Self {
            aspect: width as f32 / height as f32,
            fovy: 45.0_f32.to_radians(),
            znear: 0.1,
            zfar: 280.0,

            controller: FlyingCamera::default(),
        }
    }

    pub fn handle_event(&mut self, event: &WindowEvent) -> bool {
        self.controller.handle_event(event)
    }

    pub fn resize(&mut self, (width, height): (u32, u32)) {
        self.aspect = width as f32 / height as f32;
    }

    pub fn update(&mut self, dt: Duration) {
        self.controller.update(dt);
    }
}

impl From<&MyCamera> for Camera {
    fn from(camera: &MyCamera) -> Camera {
        Camera {
            view: camera.controller.transform.inverse(),
            proj: glam::Mat4::perspective_rh(camera.fovy, camera.aspect, camera.znear, camera.zfar),
        }
    }
}

use controller::*;
mod controller {
    use std::f32::consts::FRAC_PI_2;
    use std::time::Duration;
    use winit::{
        dpi::PhysicalPosition,
        event::*,
        keyboard::{KeyCode, PhysicalKey},
    };

    pub struct FlyingCamera {
        pub transform: glam::Mat4,

        pub speed: f32,
        pub sensitivity: f32,

        amount_left: f32,
        amount_right: f32,
        amount_forward: f32,
        amount_backward: f32,
        amount_up: f32,
        amount_down: f32,

        mouse_dx: f32,
        mouse_dy: f32,

        last_mouse_pos: PhysicalPosition<f32>,
        mouse_pressed: bool,
    }

    impl Default for FlyingCamera {
        fn default() -> Self {
            Self {
                transform: glam::Mat4::default(),

                speed: 64.0,
                sensitivity: 0.003,

                amount_left: 0.0,
                amount_right: 0.0,
                amount_forward: 0.0,
                amount_backward: 0.0,
                amount_up: 0.0,
                amount_down: 0.0,

                mouse_dx: 0.0,
                mouse_dy: 0.0,

                last_mouse_pos: (0.0, 0.0).into(),
                mouse_pressed: false,
            }
        }
    }

    impl FlyingCamera {
        pub fn handle_event(&mut self, event: &WindowEvent) -> bool {
            match event {
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            state,
                            physical_key: PhysicalKey::Code(keycode),
                            ..
                        },
                    ..
                } => {
                    let amount = if *state == ElementState::Pressed {
                        1.0
                    } else {
                        0.0
                    };

                    match keycode {
                        KeyCode::KeyW | KeyCode::ArrowUp => {
                            self.amount_forward = amount;
                            true
                        }
                        KeyCode::KeyA | KeyCode::ArrowLeft => {
                            self.amount_left = amount;
                            true
                        }
                        KeyCode::KeyS | KeyCode::ArrowDown => {
                            self.amount_backward = amount;
                            true
                        }
                        KeyCode::KeyD | KeyCode::ArrowRight => {
                            self.amount_right = amount;
                            true
                        }
                        KeyCode::KeyE => {
                            self.amount_up = amount;
                            true
                        }
                        KeyCode::KeyQ => {
                            self.amount_down = amount;
                            true
                        }
                        _ => false,
                    }
                }

                WindowEvent::MouseInput { state, .. } => {
                    self.mouse_pressed = *state == ElementState::Pressed;
                    true
                }

                WindowEvent::CursorMoved { position, .. } => {
                    self.mouse_dx = position.x as f32 - self.last_mouse_pos.x;
                    self.mouse_dy = self.last_mouse_pos.y - position.y as f32;
                    self.last_mouse_pos = (position.x, position.y).into();
                    true
                }
                _ => false,
            }
        }

        pub fn update(&mut self, dt: Duration) {
            let dt = dt.as_secs_f32();

            let matrix = self.transform.as_mut();

            let mut right = glam::vec3(matrix[0], matrix[1], matrix[2]);
            let mut back = glam::vec3(matrix[8], matrix[9], matrix[10]);

            let mut movement = glam::Vec3::ZERO;
            movement += back * (self.amount_backward - self.amount_forward);
            movement += right * (self.amount_right - self.amount_left);
            movement += back.cross(right) * (self.amount_up - self.amount_down);
            movement *= self.speed * dt;

            matrix[12] += movement.x;
            matrix[13] += movement.y;
            matrix[14] += movement.z;

            if self.mouse_pressed {
                let mut yaw = back.x.atan2(back.z);
                let mut pitch = back.y.asin();

                yaw -= self.sensitivity * self.mouse_dx;
                pitch -= self.sensitivity * self.mouse_dy;

                pitch = pitch.clamp(-FRAC_PI_2, FRAC_PI_2);

                back.x = pitch.cos() * yaw.sin();
                back.y = pitch.sin();
                back.z = pitch.cos() * yaw.cos();
                back = back.normalize();

                let world_up = glam::vec3(0.0, 1.0, 0.0);
                right = -back.cross(world_up).normalize();
                let up = back.cross(right).normalize();

                matrix[0] = right.x;
                matrix[1] = right.y;
                matrix[2] = right.z;

                matrix[4] = up.x;
                matrix[5] = up.y;
                matrix[6] = up.z;

                matrix[8] = back.x;
                matrix[9] = back.y;
                matrix[10] = back.z;
            }

            self.mouse_dx = 0.0;
            self.mouse_dy = 0.0;
        }
    }
}
