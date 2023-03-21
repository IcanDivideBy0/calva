// use std::mem::{discriminant, Discriminant};

// pub trait Slot<T> {}

// pub trait Node {
//     type Inputs;
//     type Outputs;
// }

// ////////////////////////////////////////////////////////////////////////////////////////////////////////

// pub struct GeometryOutputs {
//     pub albedo: u32,
//     pub normal: f32,
// }

// pub struct Geometry {
//     pub outputs: GeometryOutputs,
// }

// impl Geometry {
//     pub fn new() -> Self {
//         Self {
//             outputs: GeometryOutputs {
//                 albedo: 0,
//                 normal: 0.0,
//             },
//         }
//     }
// }

// pub struct AmbientInputs {
//     pub albedo: u32,
//     pub normal: f32,
// }

// pub struct Ambient {
//     pub inputs: AmbientInputs,
// }

// impl Ambient {
//     pub fn new(inputs: AmbientInputs) -> Self {
//         Self { inputs }
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test() {
//         let g = Geometry::new();
//         let _a = Ambient::new(AmbientInputs {
//             albedo: g.outputs.albedo,
//             normal: g.outputs.normal,
//         });
//     }
// }
