import super::animation::AnimationState;

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
