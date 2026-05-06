use winit::event::WindowEvent;

#[derive(Default)]
pub struct PlayerController {}

impl PlayerController {
    pub fn handle_event(&mut self, _event: &WindowEvent) -> bool {
        false
    }
}
