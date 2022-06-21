struct MeshInstance {
    model_matrix: mat4x4<f32>,
    normal_quat: vec4<f32>,
};

struct SkinAnimationInstance {
    frame: u32,
};

@group(0) @binding(0) var<storage, read_write> mesh_instances: array<MeshInstance>;
@group(0) @binding(1) var<storage, read_write> animation_instances: array<SkinAnimationInstance>;

struct SkinAnimation {
    offset: u32,
    length: u32,
};

@group(1) @binding(1) var<storage, read> animations: array<SkinAnimation>;

@compute @workgroup_size(100)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let animation_instance = &animation_instances[global_id.x];

    if ((*animation_instance).frame == !0u) {
        (*animation_instance).frame = animations[global_id.x].offset;
    }

    var offset: u32;
    var length: u32;
    for (var i = 0u; i < arrayLength(&animations); i = i + 1u) {
        let animation = &animations[i];

        if (
            (*animation_instance).frame >= (*animation).offset &&
            (*animation_instance).frame - (*animation).offset < (*animation).length
        ) {
            offset = (*animation).offset;
            length = (*animation).length;
        }
    }

    let current_frame_relative = (*animation_instance).frame - offset;
    (*animation_instance).frame = (current_frame_relative + 1u) % length + offset;

    let dz = 5.0 * 1.0 / 60.0;
    let mesh_instance = &mesh_instances[global_id.x];
    let mesh_instance_z = &(*mesh_instance).model_matrix[3][2];
    *mesh_instance_z = (*mesh_instance_z + dz + 20.0) % 40.0 - 20.0;
}