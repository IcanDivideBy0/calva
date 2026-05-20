use anyhow::Result;
use calva::renderer::{Camera, Resource, ResourcesManager, Time};
use glam::Vec3Swizzles;

use crate::worldgen::WorldGenerator;

pub struct TopDownCamera {
    pub target: glam::Vec3,
    current: glam::Vec3,
}

impl Resource for TopDownCamera {
    fn instanciate(_resources: &ResourcesManager) -> Result<Self> {
        Ok(TopDownCamera {
            target: glam::Vec3::ZERO,
            current: glam::Vec3::ZERO,
        })
    }

    fn update(&mut self, resources: &ResourcesManager) -> Result<()> {
        let worldgen = resources.read::<WorldGenerator>();
        let time = resources.read::<Time>();

        let mut camera = resources.write::<Camera>();

        let dir = self.target - self.current;
        let speed = 8.0; // units / sec
        let translation = dir * speed * time.dt.as_secs_f32();

        self.current += translation;

        if let Some(height) = worldgen.get_height(self.current.xz()) {
            self.target.y = height;
        }

        let eye = self.current + glam::vec3(16.0, 40.0, 8.0);
        // let eye = self.current + glam::vec3(0.0, 140.0, 1.0);

        camera.view = glam::Mat4::look_at_rh(eye, self.current, glam::Vec3::Y);

        Ok(())
    }
}
