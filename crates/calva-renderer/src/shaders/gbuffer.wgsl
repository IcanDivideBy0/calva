// Vertex shader

[[block]]
struct CameraUniforms {
    view_proj: mat4x4<f32>;
};

[[group(0), binding(0)]]
var<uniform> camera: CameraUniforms;

struct InstanceInput {
    [[location(0)]] model_matrix_0: vec4<f32>;
    [[location(1)]] model_matrix_1: vec4<f32>;
    [[location(2)]] model_matrix_2: vec4<f32>;
    [[location(3)]] model_matrix_3: vec4<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] color: vec3<f32>;
};

[[stage(vertex)]]
fn main(
    instance: InstanceInput,
    [[location(4)]] position: vec3<f32>
) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    var out: VertexOutput;
    out.color = vec3<f32>(1.0, 0.0, 0.0);
    out.clip_position = camera.view_proj * model_matrix * vec4<f32>(position, 1.0);

    return out;
}

// Fragment shader

struct FragmentOutput {
    [[location(0)]] albedo: vec4<f32>;
    [[location(1)]] position: vec4<f32>;
    [[location(2)]] normal: vec4<f32>;
};

[[stage(fragment)]]
fn main(in: VertexOutput) ->  FragmentOutput {
    var out: FragmentOutput;
    
    out.albedo = vec4<f32>(in.color, 1.0);
    out.position = vec4<f32>(0.1, 0.1, 0.1, 1.0);
    out.normal = vec4<f32>(0.1, 0.1, 0.1, 1.0);

    return out;
}
