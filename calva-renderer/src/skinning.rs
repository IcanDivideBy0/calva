pub struct Skin {
    pub joints: Vec<Joint>,
}

pub struct Joint {
    pub inv_bind: glam::Mat4,
}
