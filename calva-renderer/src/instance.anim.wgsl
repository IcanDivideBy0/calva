struct AnimationState {
    animation_id: u32,
    time: f32,
}

struct Instance {
    transform: mat4x4<f32>,
    mesh_id: u32,
    material_id: u32,
    animation: AnimationState,
}
struct Instances {
    count: u32,
    instances: array<Instance>,
}

@group(0) @binding(0)
var<storage, read_write> instances: Instances;

var<push_constant> time: f32;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    if global_id.x >= instances.count { return; }

    instances.instances[global_id.x].animation.time += time;
}
