import package::resources::instances::Instances;

@group(0) @binding(0) var<storage, read_write> instances: Instances;
@group(1) @binding(0) var<uniform> time: f32;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    if global_id.x >= instances.count { return; }

    instances.instances[global_id.x].animation.time += time;
}
