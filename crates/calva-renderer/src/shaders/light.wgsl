// Vertex shader

[[block]]
struct CameraUniforms {
    view: mat4x4<f32>;
    proj: mat4x4<f32>;
    view_proj: mat4x4<f32>;
};

[[group(0), binding(0)]]
var<uniform> camera: CameraUniforms;

struct InstanceInput {
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] radius: f32;
    [[location(2)]] color: vec3<f32>;
};

struct VertexInput {
    [[location(3)]] position: vec3<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] radius: f32;
    [[location(2)]] color: vec3<f32>;
};

[[stage(vertex)]]
fn main(
    instance: InstanceInput,
    in: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = in.position * instance.radius + instance.position;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);

    out.position = instance.position;
    out.radius = instance.radius;
    out.color = instance.color;

    return out;
}

// Fragment shader

[[group(1), binding(0)]] var gbuffer_albedo: texture_2d<f32>;
[[group(1), binding(1)]] var gbuffer_position: texture_2d<f32>;
[[group(1), binding(2)]] var gbuffer_normal: texture_2d<f32>;

fn compute_attenuation(light_range: f32, light_dist: f32) -> f32 {
    // TODO: maybe use this https://learnopengl.com/Lighting/Light-casters
    //
    // float attenuation = 1.0 / (u_Light.Constant + u_Light.Linear * light_dist + 
    //                     u_Light.Quadratic * (light_dist * light_dist));
    //
    // Range    Constant    Linear    Quadratic
    // 3250     1.0         0.0014    0.000007
    // 600      1.0         0.007     0.0002
    // 325      1.0         0.014     0.0007
    // 200      1.0         0.022     0.0019
    // 160      1.0         0.027     0.0028
    // 100      1.0         0.045     0.0075
    // 65       1.0         0.07      0.017
    // 50       1.0         0.09      0.032
    // 32       1.0         0.14      0.07
    // 20       1.0         0.22      0.20
    // 13       1.0         0.35      0.44
    // 7        1.0         0.7       1.8

    return 1.0 - smoothStep(light_range / 3.0, light_range, light_dist);
}

[[stage(fragment)]]
fn main(in: VertexOutput) ->  [[location(0)]] vec4<f32> {
    // if (true) { return vec4<f32>(in.color, 0.3); }

    let c = vec2<i32>(floor(in.clip_position.xy));

    let albedo = textureLoad(gbuffer_albedo, c, 0).rgb;
    let position = textureLoad(gbuffer_position, c, 0).rgb;
    let normal = textureLoad(gbuffer_normal, c, 0).rgb;

    let ambient_strength = 0.1;
    let ambient_color = in.color * ambient_strength;

    let light_to_point = position - in.position;

    let light_dir = normalize(light_to_point);
    let light_dist = length(light_to_point);

    let n_dot_l = max(dot(normal, -light_dir), 0.0);

    let diffuse_color = in.color * n_dot_l;

    // let specular_strength = pow(max(dot(normal, -light_dir), 0.0), 32.0);
    // let specular_color = specular_strength * light.color;

    let result = (ambient_color + diffuse_color) * albedo.xyz;

    let attenuation = compute_attenuation(in.radius, light_dist);
    return vec4<f32>(result, attenuation);



    // let diffuse = n_dot_l * albedo * in.color * attenuation;

    // return vec4<f32>(diffuse, 1.0);

    // return vec4<f32>(in.color, 1.0);

    // return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
