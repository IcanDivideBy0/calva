use winit::dpi::PhysicalSize;

#[derive(Debug, Copy, Clone)]
pub struct Perspective {
    pub fovy: f32, // rad
    pub aspect: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Perspective {
    pub fn new(
        size: PhysicalSize<u32>,
        fovy: f32, // deg
        znear: f32,
        zfar: f32,
    ) -> Self {
        Self {
            fovy: fovy.to_radians(),
            aspect: size.width as f32 / size.height as f32,
            znear,
            zfar,
        }
    }
}

impl Perspective {
    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.aspect = size.width as f32 / size.height as f32;
    }
}

impl From<Perspective> for glam::Mat4 {
    fn from(p: Perspective) -> glam::Mat4 {
        glam::Mat4::perspective_rh(p.fovy, p.aspect, p.znear, p.zfar)
    }
}
