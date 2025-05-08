import super::animation::AnimationState;

struct Instance {
    _padding: u32,
    /* | deleted  | mat_id   | mesh_id          | */
    /* | 8        | 8        | 16               | */
    packed_data: u32,
    animation: AnimationState,
    transform: mat4x4<f32>,
}

struct Instances {
    count: u32,
    instances: array<Instance>,
}

fn instance_deleted(instance: Instance) -> bool {
    return bool(instance.packed_data >> 24);
}

fn instance_mesh_id(instance: Instance) -> u32 {
    return instance.packed_data & 0xFFFF;
}

fn instance_material_id(instance: Instance) -> u32 {
    return (instance.packed_data >> 16) & 0xFF;
}