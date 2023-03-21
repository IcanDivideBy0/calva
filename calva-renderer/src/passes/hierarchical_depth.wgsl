@group(0) @binding(0) var t_sampler: sampler;
@group(0) @binding(1) var t_depth: texture_depth_2d;
@group(0) @binding(2) var t_output: texture_storage_2d<r32float, write>;

var<workgroup> workgroup_min: atomic<u32>;

@compute @workgroup_size(4, 4, 1)
fn main(
    @builtin(workgroup_id) wg_id : vec3<u32>,
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(local_invocation_index) local_index: u32,
) {
    if (local_index == 0u) {
        workgroup_min = bitcast<u32>(1.0);
    }

    workgroupBarrier();

    let texelSize =  1.0 / vec2<f32>(textureDimensions(t_depth));
    let base_coord = (4.0 * vec2<f32>(global_id.xy) + 0.5) / vec2<f32>(textureDimensions(t_depth));

    // let gather1 = textureGather(t_depth, t_sampler, base_coord, vec2<i32>(1, 1));
    // let gather2 = textureGather(t_depth, t_sampler, base_coord, vec2<i32>(1, 3));
    // let gather3 = textureGather(t_depth, t_sampler, base_coord, vec2<i32>(3, 1));
    // let gather4 = textureGather(t_depth, t_sampler, base_coord, vec2<i32>(3, 3));

    let gather1 = textureGather(t_depth, t_sampler, base_coord, vec2<i32>(1, 1));
    let gather2 = textureGather(t_depth, t_sampler, base_coord, vec2<i32>(1, 3));
    let gather3 = textureGather(t_depth, t_sampler, base_coord, vec2<i32>(3, 1));
    let gather4 = textureGather(t_depth, t_sampler, base_coord, vec2<i32>(3, 3));

    let gather_min = min(
        min(gather1, gather2),
        min(gather3, gather4),
    );

    let final_min = min(
        min(gather_min.x, gather_min.y),
        min(gather_min.z, gather_min.w),
    );

    atomicMin(&workgroup_min, bitcast<u32>(final_min));

    workgroupBarrier();

    if (local_index == 0u) {
        let min_depth = bitcast<f32>(workgroup_min);
        textureStore(t_output, wg_id.xy, vec4<f32>(min_depth));
	}
}