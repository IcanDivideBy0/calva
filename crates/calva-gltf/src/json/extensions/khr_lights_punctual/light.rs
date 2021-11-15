use serde::Deserialize;

fn default_color() -> glam::Vec3 {
    glam::vec3(1.0, 1.0, 1.0)
}

fn default_intensity() -> f32 {
    1.0
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Light {
    Spot {
        #[serde(default)]
        name: String,
        #[serde(default = "default_color")]
        color: glam::Vec3,
        #[serde(default = "default_intensity")]
        intensity: f32,

        range: Option<f32>,

        spot: SpotLightParams,
    },
    Directional {
        #[serde(default)]
        name: String,
        #[serde(default = "default_color")]
        color: glam::Vec3,
        #[serde(default = "default_intensity")]
        intensity: f32,
    },
    Point {
        #[serde(default)]
        name: String,
        #[serde(default = "default_color")]
        color: glam::Vec3,
        #[serde(default = "default_intensity")]
        intensity: f32,

        range: Option<f32>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotLightParams {
    #[serde(rename = "innerConeAngle")]
    pub inner_cone_angle: f32,
    #[serde(rename = "outerConeAngle")]
    pub outer_cone_angle: f32,
}

impl SpotLightParams {
    pub fn _get_angle_scale_offset(&self) -> (f32, f32) {
        // https://github.com/KhronosGroup/glTF/tree/master/extensions/2.0/Khronos/KHR_lights_punctual#inner-and-outer-cone-angles
        let light_angle_scale =
            1.0 / (0.001f32).max(self.inner_cone_angle.cos() - self.outer_cone_angle.cos());
        let light_angle_offset = -self.outer_cone_angle.cos() * light_angle_scale;

        (light_angle_scale, light_angle_offset)
    }
}
