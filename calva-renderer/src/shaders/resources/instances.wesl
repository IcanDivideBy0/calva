import super::animation::AnimationState;

struct Instance {
    /*
     * |                packed_data                    |
     * |-----------------------------------------------|
     * | X | padding             | instance_id         |
     * |   | u16                 | u16                 |
     * |---|-------------------------------------------|
     * | Y | deleted  | mat_id   | mesh_id             |
     * |   | u8       | u8       | u16                 |
     */
    packed_data: vec2<u32>,

    animation: AnimationState,
    transform: mat4x4<f32>,
}

struct Instances {
    count: u32,
    instances: array<Instance>,
}

fn instance_id(instance: Instance) -> u32 {
    return instance.packed_data.x & 0xFFFF;
}

fn instance_deleted(instance: Instance) -> bool {
    return bool(instance.packed_data.y >> 24);
}

fn instance_mesh_id(instance: Instance) -> u32 {
    return instance.packed_data.y & 0xFFFF;
}

fn instance_material_id(instance: Instance) -> u32 {
    return (instance.packed_data.y >> 16) & 0xFF;
}

//
// Compute shader
//

@group(0) @binding(0)
var<storage, read> updates: Instances;

@group(0) @binding(1)
var<storage, read_write> instances: Instances;

@compute @workgroup_size(32)
fn maintain(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let update_index = global_id.x;

    if update_index >= updates.count {
        return;
    }

    let update = &updates.instances[update_index];
    let instance_index = instance_id(*update);

    instances.instances[instance_index] = *update;
}
