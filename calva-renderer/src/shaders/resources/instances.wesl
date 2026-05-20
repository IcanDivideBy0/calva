import package::resources::animation::AnimationState;

struct Instance {
    /*
     * |                packed_data                    |
     * |-----------------------------------------------|
     * | X | instance_id         | padding             |
     * |   | u16                 | u16                 |
     * |---|-------------------------------------------|
     * | Y | flags    | mat_id   | mesh_id             |
     * |   | u8       | u8       | u16                 |
     * 
     * with flags:
     * 0b00000000
     *          ^- is_active
     *         ^-- update_transform
     *        ^--- update_animation
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
    return instance.packed_data.x >> 16;
}

fn instance_active(instance: Instance) -> bool {
    return bool((instance.packed_data.y >> 24) & (1u << 0));
}
fn instance_update_transform(instance: Instance) -> bool {
    return bool((instance.packed_data.y >> 24) & (1u << 1));
}
fn instance_update_animation(instance: Instance) -> bool {
    return bool((instance.packed_data.y >> 24) & (1u << 2));
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

@compute @workgroup_size(256)
fn update(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let update_index = global_id.x;

    if update_index >= updates.count {
        return;
    }

    let update = updates.instances[update_index];
    let instance_index = instance_id(update);
    let instance = &instances.instances[instance_index];

    let was_active = instance_active(*instance);
    let is_active = instance_active(update);
    let prev_mesh_id = instance_mesh_id(*instance);
    let new_mesh_id = instance_mesh_id(update);

    // TODO: this could be solved with prefix sum of the deltas added
    // the actual base_instances but it's kinda complex to implement
    if was_active != is_active || prev_mesh_id != new_mesh_id {
        if was_active {
            for (var i: u32 = prev_mesh_id + 1; i < arrayLength(&base_instances); i++) {
                atomicSub(&base_instances[i], 1u);
            }
        }

        if is_active {
            for (var i: u32 = new_mesh_id + 1; i < arrayLength(&base_instances); i++) {
                atomicAdd(&base_instances[i], 1u);
            }
        }
    }

    atomicMax(&instances.count, instance_index + 1);

    let prev_transform = instance.transform;
    let prev_animation = instance.animation;

    *instance = update;

    if !instance_update_transform(update) {
        instance.transform = prev_transform;
    }
    if !instance_update_animation(update) {
        instance.animation = prev_animation;
    }
}
