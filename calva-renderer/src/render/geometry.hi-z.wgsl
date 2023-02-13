@group(0) @binding(0) var t_depth: texture_depth_multisampled_2d;
@group(0) @binding(1) var t_sampler: sampler;

@group(0) @binding(2) var t_output: binding_array<texture_storage_2d<r32float, read_write> >;

var<workgroup> min_depth: atomic<u32>;

var<push_constant> mip_level: u32;

@compute @workgroup_size(4, 4, 1)
fn main(
  @builtin(workgroup_id) wg_id : vec3<u32>,
  @builtin(global_invocation_id) global_id: vec3<u32>,
  @builtin(local_invocation_id) local_id: vec3<u32>,
  @builtin(local_invocation_index) local_index: u32,
) {
  if local_index == 0u {
    min_depth = bitcast<u32>(1.0);
  }

  storageBarrier();

  var final_min: f32 = 1.0;

  if (mip_level == 0u) {
    for (var i = 0; i < textureNumSamples(t_depth); i++) {
      final_min = min(final_min, textureLoad(t_depth, global_id.xy, i));
    }
    textureStore(t_output[mip_level], global_id.xy, vec4<f32>(final_min));
    return;
  } else {
    let input = t_output[mip_level - 1u];

    let texel_dimensions = 1.0 / vec2<f32>(textureDimensions(input));
    let uv = vec2<f32>(global_id.xy) * texel_dimensions;

    let gather = vec4<f32>(
      textureLoad(input, 2u * global_id.xy + vec2<u32>(0u, 0u)).x,
      textureLoad(input, 2u * global_id.xy + vec2<u32>(0u, 1u)).x,
      textureLoad(input, 2u * global_id.xy + vec2<u32>(1u, 0u)).x,
      textureLoad(input, 2u * global_id.xy + vec2<u32>(1u, 1u)).x,
    );
    let final_min = min(
      min(gather.x, gather.y),
      min(gather.z, gather.w),
    );

    textureStore(t_output[mip_level], global_id.xy, vec4<f32>(final_min));
    return;

    // let base_texel_coords = 4u * global_id.xy;
    // let texture_dimensions = textureDimensions(t_depth);
    // let texel_dimensions = 1.0 / vec2<f32>(texture_dimensions);

    // let gather_coords_1 = vec2<f32>(base_texel_coords + vec2<u32>(1u, 1u)) * texel_dimensions;
    // let gather_coords_2 = vec2<f32>(base_texel_coords + vec2<u32>(3u, 1u)) * texel_dimensions;
    // let gather_coords_3 = vec2<f32>(base_texel_coords + vec2<u32>(1u, 3u)) * texel_dimensions;
    // let gather_coords_4 = vec2<f32>(base_texel_coords + vec2<u32>(3u, 3u)) * texel_dimensions;

    // let texel_values_1 = textureGather(t_depth, t_sampler, gather_coords_1);
    // let texel_values_2 = textureGather(t_depth, t_sampler, gather_coords_2);
    // let texel_values_3 = textureGather(t_depth, t_sampler, gather_coords_3);
    // let texel_values_4 = textureGather(t_depth, t_sampler, gather_coords_4);

    // let gathered_min = min(
    //   min(texel_values_1, texel_values_2),
    //   min(texel_values_3, texel_values_4),
    // );

    // final_min = min(
    //   min(gathered_min.x, gathered_min.y),
    //   min(gathered_min.z, gathered_min.w),
    // );

    // final_min = gathered_min;
    // final_min = 1.0;
  }

  // atomicMin(&min_depth, bitcast<u32>(final_min));

  // storageBarrier();

  // if local_index == 0u {
  //   let workgroup_min_depth = bitcast<f32>(min_depth);
  //   textureStore(t_output[mip_level], global_id.xy, vec4<f32>(workgroup_min_depth));
  // }
}