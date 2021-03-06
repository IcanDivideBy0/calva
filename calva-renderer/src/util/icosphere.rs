use std::collections::HashMap;

pub struct Icosphere {
    pub vertices: Vec<glam::Vec3>,
    pub indices: Vec<u16>,
    pub count: u32,
}

impl Icosphere {
    #[allow(clippy::many_single_char_names)]
    pub fn new(order: u32) -> Self {
        // set up a 20-triangle icosahedron
        let f = (1.0 + 5.0_f32.powf(0.5)) / 2.0;

        #[rustfmt::skip]
        let mut vertices = vec![
            glam::vec3(-1.0,    f,  0.0),
            glam::vec3( 1.0,    f,  0.0),
            glam::vec3(-1.0,   -f,  0.0),
            glam::vec3( 1.0,   -f,  0.0),

            glam::vec3( 0.0, -1.0,    f),
            glam::vec3( 0.0,  1.0,    f),
            glam::vec3( 0.0, -1.0,   -f),
            glam::vec3( 0.0,  1.0,   -f),

            glam::vec3(   f,  0.0, -1.0),
            glam::vec3(   f,  0.0,  1.0),
            glam::vec3(  -f,  0.0, -1.0),
            glam::vec3(  -f,  0.0,  1.0),
        ];

        #[rustfmt::skip]
        let mut indices = vec![
             0, 11,  5,
             0,  5,  1,
             0,  1,  7,
             0,  7, 10,
             0, 10, 11,
            11, 10,  2,
             5, 11,  4,
             1,  5,  9,
             7,  1,  8,
            10,  7,  6,
             3,  9,  4,
             3,  4,  2,
             3,  2,  6,
             3,  6,  8,
             3,  8,  9,
             9,  8,  1,
             4,  9,  5,
             2,  4, 11,
             6,  2, 10,
             8,  6,  7,
        ];

        let mut v: u16 = 12;
        let mut mid_cache: HashMap<(u16, u16), u16> = HashMap::new();
        let mut add_mid_point = move |vertices: &mut Vec<glam::Vec3>, a: u16, b: u16| -> u16 {
            let key = (a, b);

            match mid_cache.get(&key).copied() {
                Some(index) => index,
                None => {
                    mid_cache.insert(key, v);

                    let mid_point = (vertices[a as usize] + vertices[b as usize]) / 2.0;
                    vertices.push(mid_point);

                    v += 1;
                    v - 1
                }
            }
        };

        let mut indices_prev = indices.clone();
        for _ in 0..order {
            // Subdivide each triangle into 4 triangles
            indices = vec![0; indices_prev.len() * 4];

            for k in (0..indices_prev.len()).step_by(3) {
                let v1 = indices_prev[k];
                let v2 = indices_prev[k + 1];
                let v3 = indices_prev[k + 2];
                let a = add_mid_point(&mut vertices, v1, v2);
                let b = add_mid_point(&mut vertices, v2, v3);
                let c = add_mid_point(&mut vertices, v3, v1);

                let mut t = (k * 4)..;
                indices[t.next().unwrap()] = v1;
                indices[t.next().unwrap()] = a;
                indices[t.next().unwrap()] = c;
                indices[t.next().unwrap()] = v2;
                indices[t.next().unwrap()] = b;
                indices[t.next().unwrap()] = a;
                indices[t.next().unwrap()] = v3;
                indices[t.next().unwrap()] = c;
                indices[t.next().unwrap()] = b;
                indices[t.next().unwrap()] = a;
                indices[t.next().unwrap()] = b;
                indices[t.next().unwrap()] = c;
            }

            indices_prev = indices.clone();
        }

        // Normalize vertices
        let vertices = vertices
            .drain(..)
            .map(|v| v.normalize())
            .collect::<Vec<_>>();

        let count = indices.len() as u32;

        Icosphere {
            vertices,
            indices,
            count,
        }
    }
}
