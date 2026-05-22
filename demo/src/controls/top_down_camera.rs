use anyhow::Result;
use calva::renderer::{Camera, Resource, ResourcesManager, Time};
use tween::{QuadOut, Tweener};

pub struct TopDownCamera {
    target: glam::Vec3,
    tweener: Tweener<glam::Vec3, f32, QuadOut>,
}

impl TopDownCamera {
    pub fn get_target(&self) -> glam::Vec3 {
        self.target
    }

    pub fn set_target(&mut self, target: glam::Vec3) {
        self.tweener = Tweener::quad_out(self.target, target, 0.6);
        self.target = target
    }
}

impl Resource for TopDownCamera {
    fn instanciate(_resources: &ResourcesManager) -> Result<Self> {
        Ok(TopDownCamera {
            target: glam::Vec3::ZERO,
            tweener: Tweener::quad_out(glam::Vec3::ZERO, glam::Vec3::ZERO, 0.0),
        })
    }

    fn update(&mut self, resources: &ResourcesManager) -> Result<()> {
        let time = resources.read::<Time>();
        let mut camera = resources.write::<Camera>();

        let target = self.tweener.move_by(time.dt.as_secs_f32());
        let eye = target + glam::vec3(16.0, 40.0, 8.0);

        camera.view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::Y);

        Ok(())
    }
}
