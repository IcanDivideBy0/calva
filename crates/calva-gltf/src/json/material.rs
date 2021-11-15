use serde::{Deserialize, Deserializer};

use super::{Document, Texture};

fn default_emissive_factor() -> glam::Vec3 {
    glam::vec3(0.0, 0.0, 0.0)
}

fn default_alpha_mode() -> MaterialAlphaMode {
    MaterialAlphaMode::Opaque
}

fn default_alpha_cutoff() -> f32 {
    0.5
}

fn default_double_sided() -> bool {
    false
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    pub name: String,
    pub pbr_metallic_roughness: Option<PbrMetallicRoughness>,

    pub normal_texture: Option<NormalTexture>,
    pub occlusion_texture: Option<MaterialTexture>,
    pub emissive_texture: Option<MaterialTexture>,

    #[serde(default = "default_emissive_factor")]
    pub emissive_factor: glam::Vec3,
    #[serde(default = "default_alpha_mode")]
    pub alpha_mode: MaterialAlphaMode,
    #[serde(default = "default_alpha_cutoff")]
    pub alpha_cutoff: f32,
    #[serde(default = "default_double_sided")]
    pub double_sided: bool,
}

fn default_tex_coord() -> i32 {
    0
}

fn default_normal_texture_scale() -> f32 {
    1.0
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalTexture {
    pub index: usize,
    #[serde(default = "default_tex_coord")]
    pub tex_coord: i32,
    #[serde(default = "default_normal_texture_scale")]
    pub scale: f32,
}

impl NormalTexture {
    pub fn texture<'a: 'b, 'b>(&'a self, doc: &'b Document) -> &'b Texture {
        doc.textures.get(self.index).unwrap()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialTexture {
    pub index: usize,
    #[serde(default = "default_tex_coord")]
    pub tex_coord: i32,
}

impl MaterialTexture {
    pub fn texture<'a: 'b, 'b>(&'a self, doc: &'b Document) -> &'b Texture {
        doc.textures.get(self.index).unwrap()
    }
}

fn default_base_color_factor() -> glam::Vec4 {
    glam::vec4(1.0, 1.0, 1.0, 1.0)
}

fn default_metallic_factor() -> f32 {
    1.0
}

fn default_roughness_factor() -> f32 {
    1.0
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PbrMetallicRoughness {
    #[serde(default = "default_base_color_factor")]
    pub base_color_factor: glam::Vec4,
    #[serde(default = "default_metallic_factor")]
    pub metallic_factor: f32,
    #[serde(default = "default_roughness_factor")]
    pub roughness_factor: f32,

    pub base_color_texture: Option<MaterialTexture>,
    pub metallic_roughness_texture: Option<MaterialTexture>,
}

#[derive(Debug, PartialEq)]
pub enum MaterialAlphaMode {
    Opaque,
    Mask,
    Blend,
}

impl<'de> Deserialize<'de> for MaterialAlphaMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "OPAQUE" => Ok(Self::Opaque),
            "MASK" => Ok(Self::Mask),
            "BLEND" => Ok(Self::Blend),

            value => Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(value),
                &r#"one of ["OPAQUE" ,"MASK" ,"BLEND"]"#,
            )),
        }
    }
}
