// Vertex shader

[[block]]
struct CameraUniforms {
    view: mat4x4<f32>;
    proj: mat4x4<f32>;
    view_proj: mat4x4<f32>;
};

[[group(0), binding(0)]]
var<uniform> camera: CameraUniforms;

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] color: vec3<f32>;
};

[[stage(vertex)]]
fn main([[location(0)]] position: vec3<f32>) -> VertexOutput {
    return VertexOutput (
        camera.view_proj * vec4<f32>(position, 1.0),
        vec3<f32>(1.0, 0.0, 0.0),
    );
}

// Fragment shader

struct FragmentOutput {
    [[location(0)]] albedo: vec4<f32>;
    [[location(1)]] position: vec4<f32>;
    [[location(2)]] normal: vec4<f32>;
};

[[stage(fragment)]]
fn main(in: VertexOutput) ->  FragmentOutput {
    return FragmentOutput (
        vec4<f32>(in.color, 1.0),
        vec4<f32>(0.1, 0.1, 0.1, 1.0),
        vec4<f32>(0.1, 0.1, 0.1, 1.0),
    );
}
