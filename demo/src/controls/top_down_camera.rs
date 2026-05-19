use anyhow::Result;
use calva::renderer::{Camera, Resource, ResourcesManager};

use crate::worldgen::WorldGenerator;

pub struct TopDownCamera {
    pub target: glam::Vec3,
}

impl Resource for TopDownCamera {
    fn instanciate(_resources: &ResourcesManager) -> Result<Self> {
        Ok(TopDownCamera {
            target: glam::Vec3::ZERO,
        })
    }

    fn update(&mut self, resources: &ResourcesManager) -> Result<()> {
        let worldgen = resources.read::<WorldGenerator>();
        let mut camera = resources.write::<Camera>();

        if let Some(height) = worldgen.get_height(self.target) {
            self.target.y = height;
        }

        let eye = self.target + glam::vec3(16.0, 40.0, 8.0);

        camera.view = glam::Mat4::look_at_rh(eye, self.target, glam::Vec3::Y);

        Ok(())
    }
}
