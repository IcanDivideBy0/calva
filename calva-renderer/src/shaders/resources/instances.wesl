import package::resources::animation::AnimationState;

struct Instance {
    /*
     * |                packed_data                    |
     * |-----------------------------------------------|
     * | X | padding             | instance_id         |
     * |   | u16                 | u16                 |
     * |---|-------------------------------------------|
     * | Y | active   | mat_id   | mesh_id             |
     * |   | u8       | u8       | u16                 |
     */
    packed_data: vec2<u32>,

    animation: AnimationState,
    transform: mat4x4<f32>,
}

struct Instances {
    count: atomic<u32>,
    instances: array<Instance>,
}

//
// Utils fn
//

fn instance_id(instance: Instance) -> u32 {
    return instance.packed_data.x & 0xFFFF;
}

fn instance_active(instance: Instance) -> bool {
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
var<storage, read_write> base_instances: array<atomic<u32>>;

@group(0) @binding(2)
var<storage, read_write> instances: Instances;

@compute @workgroup_size(32)
fn maintain(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let update_index = global_id.x;

    if update_index >= updates.count {
        return;
    }

    let update = updates.instances[update_index];
    let instance_index = instance_id(update);
    let instance = &instances.instances[instance_index];

    if instance_active(*instance) {
        for (var i: u32 = instance_mesh_id(*instance) + 1; i < arrayLength(&base_instances); i++) {
            atomicSub(&base_instances[i], 1u);
        }
    }

    if instance_active(update) {
        for (var i: u32 = instance_mesh_id(update) + 1; i < arrayLength(&base_instances); i++) {
            atomicAdd(&base_instances[i], 1u);
        }
    }

    atomicMax(&instances.count, instance_index + 1);

    *instance = update;
}
