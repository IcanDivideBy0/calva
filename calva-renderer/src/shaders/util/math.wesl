
fn rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}

fn mat4_to_mat3(m: mat4x4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(m[0].xyz, m[1].xyz, m[2].xyz);
}

fn plane_distance_to_point(plane: vec4<f32>, p: vec3<f32>) -> f32 {
    return dot(plane.xyz, p) + plane.w;
}

fn axis_quat(x_axis: vec3<f32>, y_axis: vec3<f32>, z_axis: vec3<f32>) -> vec4<f32> {
    // Based on https://github.com/microsoft/DirectXMath `XM$quaternionRotationMatrix`
    if z_axis.z <= 0.0 {
        // x^2 + y^2 >= z^2 + w^2
        let dif10 = y_axis.y - x_axis.x;
        let omm22 = 1.0 - z_axis.z;
        if dif10 <= 0.0 {
            // x^2 >= y^2
            let four_xsq = omm22 - dif10;
            let inv4x = 0.5 / sqrt(four_xsq);
            return vec4<f32>(
                four_xsq * inv4x,
                (x_axis.y + y_axis.x) * inv4x,
                (x_axis.z + z_axis.x) * inv4x,
                (y_axis.z - z_axis.y) * inv4x,
            );
        } else {
            // y^2 >= x^2
            let four_ysq = omm22 + dif10;
            let inv4y = 0.5 / sqrt(four_ysq);
            return vec4<f32>(
                (x_axis.y + y_axis.x) * inv4y,
                four_ysq * inv4y,
                (y_axis.z + z_axis.y) * inv4y,
                (z_axis.x - x_axis.z) * inv4y,
            );
        }
    } else {
        // z^2 + w^2 >= x^2 + y^2
        let sum10 = y_axis.y + x_axis.x;
        let opm22 = 1.0 + z_axis.z;
        if sum10 <= 0.0 {
            // z^2 >= w^2
            let four_zsq = opm22 - sum10;
            let inv4z = 0.5 / sqrt(four_zsq);
            return vec4<f32>(
                (x_axis.z + z_axis.x) * inv4z,
                (y_axis.z + z_axis.y) * inv4z,
                four_zsq * inv4z,
                (x_axis.y - y_axis.x) * inv4z,
            );
        } else {
            // w^2 >= z^2
            let four_wsq = opm22 + sum10;
            let inv4w = 0.5 / sqrt(four_wsq);
            return vec4<f32>(
                (y_axis.z - z_axis.y) * inv4w,
                (z_axis.x - x_axis.z) * inv4w,
                (x_axis.y - y_axis.x) * inv4w,
                four_wsq * inv4w,
            );
        }
    }
}
