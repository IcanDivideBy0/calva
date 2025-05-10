const SHADERS: &[&str] = &[
    "passes/ambient_light",
    "passes/animate",
    "passes/directional_light[blur]",
    "passes/directional_light[cull]",
    "passes/directional_light[depth]",
    "passes/directional_light[lighting]",
    "passes/fxaa",
    "passes/geometry",
    "passes/geometry[cull]",
    "passes/hierarchical_depth",
    "passes/point_lights",
    "passes/skybox",
    "passes/ssao",
    "passes/ssao[blit]",
    "passes/ssao[blur]",
    "passes/tone_mapping",
    "resources/instances",
];

fn main() {
    for shader in SHADERS {
        wesl::Wesl::new("src/shaders")
            .build_artifact(format!("{shader}.wesl"), &shader.replace("/", "::"));
    }
}
