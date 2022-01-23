struct MeshInstance {
    model_matrix: mat4x4<f32>;
    normal_quat: vec4<f32>;
};
struct MeshInstances {
    data: array<MeshInstance>;
};

struct SkinAnimationInstance {
    frame: u32;
};
struct SkinAnimationInstances {
    data: array<SkinAnimationInstance>;
};

@group(0) @binding(0) var<storage, read_write> mesh_instances: MeshInstances;
@group(0) @binding(1) var<storage, read_write> animation_instances: SkinAnimationInstances;

struct SkinAnimation {
    offset: u32;
    length: u32;
};
struct SkinAnimations {
    data: array<SkinAnimation>;
};

@group(1) @binding(1) var<storage, read> animations: SkinAnimations;

// fn find_animation(current_frame: u32) -> ptr<storage, SkinAnimation> {}

@stage(compute) @workgroup_size(100)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let animation_instance = &animation_instances.data[global_id.x];

    if ((*animation_instance).frame == !0u) {
        (*animation_instance).frame = animations.data[global_id.x].offset;
    }

    var offset: u32;
    var length: u32;
    for (var i = 0u; i < arrayLength(&animations.data); i = i + 1u) {
        let animation = &animations.data[i];

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


    let z = 5.0 * 1.0 / 60.0;

    // let mesh_instance_size = 16u + 9u;
    // let mesh_instance_idx = global_id.x * mesh_instance_size;
    // let mesh_instance_z = &mesh_instances.data[global_id.x * mesh_instance_size + 14u];

    // let mesh_instance_z = &mesh_instances.data[global_id.x][14u];

    let mesh_instance = &mesh_instances.data[global_id.x];
    // (*mesh_instance).model_matrix = mat4x4<f32>(
    //     vec4<f32>(1.0, 0.0, 0.0, 0.0),
    //     vec4<f32>(0.0, 1.0, 0.0, 0.0),
    //     vec4<f32>(0.0, 0.0, 1.0, 0.0),
    //     vec4<f32>(0.0, 0.0, z, 1.0),
    // ) * (*mesh_instance).model_matrix;
    // let mesh_instance_z = &(*mesh_instance).model_matrix[3][2];

    // let matrix = bitcast<mat4x4<f32> >((*mesh_instance).model_matrix);

    // let mesh_instance_z = &(*mesh_instance).model_matrix[14];
    // let mesh_instance_z = &(*mesh_instance).model_matrix[3][2];
    // let mesh_instance_z = &(*mesh_instance).model_matrix_3[2];

    // *mesh_instance_z = (*mesh_instance_z + z + 20.0) % 40.0 - 20.0;
}