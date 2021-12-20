struct Camera {
    view: mat4x4<f32>;
    proj: mat4x4<f32>;
    view_proj: mat4x4<f32>;
    inv_view: mat4x4<f32>;
    inv_proj: mat4x4<f32>;
};

[[group(0), binding(0)]]
var<uniform> camera: Camera;

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

struct VertexInput {
    [[location(7)]] position: vec3<f32>;
    [[location(8)]] color: vec3<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] color: vec3<f32>;
};

[[stage(vertex)]]
fn vs_main(
    instance: InstanceInput,
    in: VertexInput,
) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    return VertexOutput (
        camera.view_proj * model_matrix * vec4<f32>(in.position, 1.0),
        in.color,
    );
}

//
// Fragment shader
//

struct FragmentOutput {
    [[location(0)]] albedo_metallic: vec4<f32>;
    [[location(1)]] normal_roughness: vec4<f32>;
};

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> FragmentOutput {
    return FragmentOutput (
        vec4<f32>(in.color, 1.0),
        vec4<f32>(0.1, 0.1, 0.1, 1.0),
    );
}
