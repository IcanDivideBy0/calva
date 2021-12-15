let CASCADES: u32 = 4u;

[[block]]
struct ShadowLight {
    color: vec4<f32>;
    direction: vec4<f32>; // camera view space
    view_proj: array<mat4x4<f32>, CASCADES>;
    splits: array<f32, CASCADES>;
};

[[group(0), binding(0)]]
var<uniform> shadow_light: ShadowLight;

//
// Vertex shader
//

struct InstanceInput {
    [[location(0)]] model_matrix_0: vec4<f32>;
    [[location(1)]] model_matrix_1: vec4<f32>;
    [[location(2)]] model_matrix_2: vec4<f32>;
    [[location(3)]] model_matrix_3: vec4<f32>;

    [[location(4)]] normal_matrix_0: vec3<f32>;
    [[location(5)]] normal_matrix_1: vec3<f32>;
    [[location(6)]] normal_matrix_2: vec3<f32>;
};

struct VertexOutput {
    [[builtin(position)]] position: vec4<f32>;
    [[location(0)]] depth: f32;
};

[[stage(vertex)]]
fn vs_main(
    [[builtin(view_index)]] view_index: i32,
    instance: InstanceInput,
    [[location(7)]] position: vec3<f32>,
) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    let light_view_proj = shadow_light.view_proj[view_index];
    let position = light_view_proj * model_matrix * vec4<f32>(position, 1.0);

    return VertexOutput(
        position,
        position.z
    );
}

//
// Fragment shader
//

[[group(2), binding(0)]] var t_skybox: texture_cube<f32>;
[[group(2), binding(1)]] var s_skybox: sampler;

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec2<f32> {
    // let moment = max(in.distance, 0.0);
    // let moment = clamp(in.depth * 0.5 + 0.5, 0.0, 1.0);
    let moment = in.depth;

    let dx = dpdx(moment);
    let dy = dpdy(moment);
    let moment2 = moment * moment + 0.25 * (dx * dx + dy * dy);

    return vec2<f32>(moment, moment2);
}
