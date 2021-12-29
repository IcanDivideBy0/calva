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

impl<T: Lerp + Copy + std::fmt::Debug> ChannelSampler<T> {
    fn first(&self) -> (&Duration, &T) {
        self.0.range(..).next().unwrap()
    }

    fn last(&self) -> (&Duration, &T) {
        self.0.range(..).rev().next().unwrap()
    }

    fn closest_before(&self, time: &Duration) -> (&Duration, &T) {
        self.0
            .range(..time)
            .next_back()
            .unwrap_or_else(|| self.first())
    }

    fn closest_after(&self, time: &Duration) -> (&Duration, &T) {
        self.0.range(time..).next().unwrap_or_else(|| self.last())
    }

    pub fn get_value(&self, time: &Duration) -> T {
        let before = self.closest_before(time);
        let after = self.closest_after(time);

        if before.0 == after.0 {
            return *before.1;
        }

        let alpha = (time.as_secs_f32() - before.0.as_secs_f32())
            / (after.0.as_secs_f32() - before.0.as_secs_f32());

        T::lerp(*before.1, *after.1, alpha)
    }

    pub fn get_time_range(&self) -> (Duration, Duration) {
        (*self.first().0, *self.last().0)
    }
}

struct NodeSampler {
    translations: ChannelSampler<glam::Vec3>,
    rotations: ChannelSampler<glam::Quat>,
    scales: ChannelSampler<glam::Vec3>,
}

impl NodeSampler {
    pub fn from_node_default(node: gltf::Node) -> Self {
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

    pub fn get_transform(&self, time: &Duration) -> glam::Mat4 {
        glam::Mat4::from_scale_rotation_translation(
            self.scales.get_value(time),
            self.rotations.get_value(time),
            self.translations.get_value(time),
        )
    }

    pub fn get_time_range(&self) -> (Duration, Duration) {
        vec![
            self.translations.get_time_range(),
            self.rotations.get_time_range(),
            self.scales.get_time_range(),
        ]
        .drain(..)
        .fold(
            (Duration::from_secs_f32(0.0), Duration::from_secs_f32(0.0)),
            |acc, (start, end)| (acc.0.min(start), acc.1.max(end)),
        )
    }
}

pub struct Animation {
    samplers: HashMap<usize, NodeSampler>,
}

impl Animation {
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

        Self { samplers }
    }

    pub fn get_node_transform(&self, node: &gltf::Node, time: &Duration) -> glam::Mat4 {
        self.samplers
            .get(&node.index())
            .map(|node_sampler| node_sampler.get_transform(time))
            .unwrap_or_else(|| glam::Mat4::from_cols_array_2d(&node.transform().matrix()))
    }

    fn apply_samplers_transforms<'a>(
        &self,
        time: &Duration,
        nodes: impl Iterator<Item = gltf::Node<'a>>,
        parent_world_transform: glam::Mat4,
        store: &mut HashMap<usize, glam::Mat4>,
    ) {
        for node in nodes {
            let local_transform = self.get_node_transform(&node, time);
            let world_transform = parent_world_transform * local_transform;

            store.insert(node.index(), world_transform);

            self.apply_samplers_transforms(time, node.children(), world_transform, store);
        }
    }

    pub fn get_nodes_transforms<'a>(
        &self,
        time: &Duration,
        nodes: impl Iterator<Item = gltf::Node<'a>>,
    ) -> HashMap<usize, glam::Mat4> {
        let mut frame_nodes_transforms = HashMap::new();

        self.apply_samplers_transforms(
            time,
            nodes,
            glam::Mat4::IDENTITY,
            &mut frame_nodes_transforms,
        );

        frame_nodes_transforms
    }

    pub fn get_time_range(&self) -> (Duration, Duration) {
        self.samplers
            .values()
            .map(NodeSampler::get_time_range)
            .fold(
                (Duration::from_secs_f32(0.0), Duration::from_secs_f32(0.0)),
                |acc, (start, end)| (acc.0.min(start), acc.1.max(end)),
            )
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
