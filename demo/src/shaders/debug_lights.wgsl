//
// Vertex shader
//

[[block]]
struct CameraUniforms {
    view: mat4x4<f32>;
    proj: mat4x4<f32>;
    view_proj: mat4x4<f32>;
};

[[group(0), binding(0)]]
var<uniform> camera: CameraUniforms;

struct InstanceInput {
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] radius: f32;
    [[location(2)]] color: vec3<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] color: vec3<f32>;
};

[[stage(vertex)]]
fn vs_main(
    instance: InstanceInput,
    [[location(3)]] position: vec3<f32>,
) -> VertexOutput {
    let scale = instance.radius / 100.0;
    let pos = scale * position + instance.position;

    return VertexOutput(
        camera.view_proj * vec4<f32>(pos, 1.0),
        instance.color,
    );
}

//
// Fragment shader
//

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
