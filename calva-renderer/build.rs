const SHADERS: &'static [&'static str] = &[
    "ambient_light",
    "animate",
    "directional_light[blur]",
    "directional_light[cull]",
    "directional_light[depth]",
    "directional_light[lighting]",
    "fxaa",
    "geometry",
    "geometry[cull]",
    "hierarchical_depth",
    "point_lights",
    "skybox",
    "ssao",
    "ssao[blit]",
    "ssao[blur]",
    "tone_mapping",
];

fn main() {
    for shader in SHADERS {
        wesl::Wesl::new("src/shaders").build_artifact(format!("{shader}.wesl"), shader);
    }
}
