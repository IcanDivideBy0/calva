use gltf::animation::util::ReadOutputs;
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use super::util;

trait Lerp {
    fn lerp(a: Self, b: Self, alpha: f32) -> Self;
}

impl Lerp for glam::Vec3 {
    fn lerp(a: Self, b: Self, alpha: f32) -> Self {
        glam::Vec3::lerp(a, b, alpha)
    }
}

impl Lerp for glam::Quat {
    fn lerp(a: Self, b: Self, alpha: f32) -> Self {
        glam::Quat::slerp(a, b, alpha)
    }
}

struct ChannelSampler<T>(BTreeMap<Duration, T>);

impl<T: Lerp + Copy> ChannelSampler<T> {
    fn get_value(&self, time: &Duration) -> Option<T> {
        let mut prev: Option<(Duration, T)> = None;

        for (&keyframe, &value) in &self.0 {
            if keyframe > *time {
                return match prev {
                    Some((prev_time, prev_value)) => {
                        let interval = keyframe.as_secs_f32() - prev_time.as_secs_f32();
                        let alpha = (time.as_secs_f32() - prev_time.as_secs_f32()) / interval;

                        Some(T::lerp(prev_value, value, alpha))
                    }
                    None => Some(value),
                };
            }

            prev = Some((keyframe, value));
        }

        prev.map(|x| x.1)
    }
}

struct NodeSampler {
    translations: ChannelSampler<glam::Vec3>,
    rotations: ChannelSampler<glam::Quat>,
    scales: ChannelSampler<glam::Vec3>,
}

impl NodeSampler {
    fn from_node_default(node: gltf::Node) -> Self {
        let (translation, rotation, scale) = node.transform().decomposed();

        let translation = glam::Vec3::from(translation);
        let rotation = glam::Quat::from_slice(&rotation);
        let scale = glam::Vec3::from(scale);

        let keyframe = Duration::from_secs_f32(0.0);

        Self {
            translations: ChannelSampler([(keyframe, translation)].into()),
            rotations: ChannelSampler([(keyframe, rotation)].into()),
            scales: ChannelSampler([(keyframe, scale)].into()),
        }
    }

    fn get_transform(&self, time: &Duration) -> glam::Mat4 {
        glam::Mat4::from_scale_rotation_translation(
            self.scales.get_value(time).unwrap(),
            self.rotations.get_value(time).unwrap(),
            self.translations.get_value(time).unwrap(),
        )
    }
}

pub struct NodeSamplers(HashMap<usize, NodeSampler>);

impl NodeSamplers {
    pub fn new(animation: gltf::Animation, buffers: &[gltf::buffer::Data]) -> Self {
        let mut samplers: HashMap<usize, NodeSampler> = HashMap::new();

        for channel in animation.channels() {
            let reader = channel.reader(util::buffer_reader(buffers));

            let keyframes = reader
                .read_inputs()
                .unwrap()
                .map(Duration::from_secs_f32)
                .collect::<Vec<_>>();

            let target_node = channel.target().node();
            let mut sampler = samplers
                .entry(target_node.index())
                .or_insert_with(|| NodeSampler::from_node_default(target_node));

            match reader.read_outputs().unwrap() {
                ReadOutputs::Translations(translations) => {
                    sampler.translations = ChannelSampler(
                        keyframes
                            .iter()
                            .copied()
                            .zip(translations.map(glam::Vec3::from))
                            .collect(),
                    );
                }
                ReadOutputs::Rotations(rotations) => {
                    let it = match rotations {
                        gltf::animation::util::Rotations::F32(it) => it,
                        _ => unimplemented!(),
                    };

                    sampler.rotations = ChannelSampler(
                        keyframes
                            .iter()
                            .copied()
                            .zip(it.map(|s| glam::Quat::from_slice(&s)))
                            .collect(),
                    );
                }
                ReadOutputs::Scales(scales) => {
                    sampler.scales = ChannelSampler(
                        keyframes
                            .iter()
                            .copied()
                            .zip(scales.map(glam::Vec3::from))
                            .collect(),
                    );
                }
                _ => unimplemented!(),
            }
        }

        Self(samplers)
    }

    #[allow(unused)]
    pub fn get_node_transform(&self, node_index: &usize, time: &Duration) -> Option<glam::Mat4> {
        self.0
            .get(node_index)
            .map(|node_sampler| node_sampler.get_transform(time))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn it_works() {
        let sampler = NodeSampler {
            translations: ChannelSampler(
                [
                    (Duration::from_secs_f32(1.0), glam::vec3(10.0, 0.0, 0.0)),
                    (Duration::from_secs_f32(2.0), glam::vec3(20.0, 0.0, 0.0)),
                ]
                .into(),
            ),
            rotations: ChannelSampler(
                [(Duration::from_secs_f32(0.0), glam::Quat::IDENTITY)].into(),
            ),
            scales: ChannelSampler([(Duration::from_secs_f32(0.0), glam::Vec3::ONE)].into()),
        };

        assert_eq!(
            sampler.get_transform(&Duration::from_secs_f32(1.3)),
            glam::Mat4::from_translation(glam::vec3(13.0, 0.0, 0.0))
        );

        assert_eq!(
            sampler.get_transform(&Duration::from_secs_f32(0.0)),
            glam::Mat4::from_translation(glam::vec3(10.0, 0.0, 0.0))
        );

        assert_eq!(
            sampler.get_transform(&Duration::from_secs_f32(3.0)),
            glam::Mat4::from_translation(glam::vec3(20.0, 0.0, 0.0))
        );
    }
}
