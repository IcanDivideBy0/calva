[[block]]
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
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] radius: f32;
    [[location(2)]] color: vec3<f32>;
};

struct VertexInput {
    [[location(3)]] position: vec3<f32>;
};

[[stage(vertex)]]
fn vs_main(
    instance: InstanceInput,
    in: VertexInput,
) -> [[builtin(position)]] vec4<f32> {
    let world_pos = in.position * instance.radius + instance.position;
    return camera.view_proj * vec4<f32>(world_pos, 1.0);
}
