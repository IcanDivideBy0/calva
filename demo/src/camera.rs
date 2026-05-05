pub struct PerspectiveCamera {
    pub aspect: f32,
    pub fovy: f32, // rad
    pub znear: f32,
    pub zfar: f32,
}

impl PerspectiveCamera {
    pub fn new((width, height): (u32, u32)) -> Self {
        Self {
            aspect: width as f32 / height as f32,
            fovy: 45.0_f32.to_radians(),
            znear: 0.1,
            zfar: 100.0,
        }
    }

    pub fn get_proj(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh(self.fovy, self.aspect, self.znear, self.zfar)
    }

    pub fn resize(&mut self, (width, height): (u32, u32)) {
        self.aspect = width as f32 / height as f32;
    }
}
