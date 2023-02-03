struct AnimationState {
    animation_id: u32,
    time: f32,
}

struct CullInstance {
    transform: mat4x4<f32>,
    mesh_id: u32,
    material_id: u32,
    animation: AnimationState,
}

@group(0) @binding(0)
var<storage, read_write> cull_instances: array<CullInstance>;

var<push_constant> time: f32;

@compute @workgroup_size(32)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    cull_instances[global_id.x].animation.time += time;
}
