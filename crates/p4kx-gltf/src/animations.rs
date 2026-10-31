// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use gltf_json as json;
use json::animation::{Channel, Interpolation, Property, Sampler, Target};
use json::validation::Checked::Valid;
use json::{Index, Root};
use p4kx_model::{Animation, Bone, Keys};

use crate::block_data::{BlockData, seconds};
use crate::document::push_accessor;
use crate::layout::Layout;

/// What an animation became in the GLB.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct AnimationSummary {
    /// The animation's name.
    pub name: String,
    /// The length of the animation in seconds.
    pub duration_seconds: f32,
    /// Whether the animation is additive: its keys are offsets to be added to a pose, which a
    /// glTF cannot say, so a viewer plays them as poses.
    pub additive: bool,
    /// The number of channels written: one per rotation or translation track of a bone.
    pub channels: usize,
    /// The number of keys written.
    pub keys: usize,
    /// The controller IDs of the tracks left out because no bone of the skeleton has them, or the
    /// bone drives no node, or a bone the animation already moves has the same controller ID. For
    /// a sampled animation they also hold the controller IDs of the joints that move but cannot
    /// be written; all are the CRC-32 of a name and are not told apart.
    pub skipped_tracks: Vec<u32>,
    /// The number of runs of morph weights left out because the piece they name has no mesh or
    /// another number of morph targets than the run holds.
    pub skipped_weights: usize,
}

/// The nodes the bones of the written skeleton or the static model's helper bones drive.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Targets<'a> {
    /// The bones the tracks are matched against.
    pub(crate) bones: &'a [Bone],
    /// The glTF node index of the first bone when the bones are written as nodes; otherwise a
    /// bone drives the model node [`Bone::node`] names.
    pub(crate) first_bone_node: Option<usize>,
    /// The number of nodes the model has.
    pub(crate) model_nodes: usize,
}

impl Targets<'_> {
    fn node(&self, bone: usize) -> Option<usize> {
        match self.first_bone_node {
            Some(first) => Some(first + bone),
            None => self
                .bones
                .get(bone)?
                .node
                .filter(|&node| node < self.model_nodes),
        }
    }
}

/// One channel of an animation: the blocks of its key times and values.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ChannelPlan {
    pub(crate) node: usize,
    pub(crate) path: ChannelPath,
    pub(crate) times: usize,
    pub(crate) values: usize,
    pub(crate) step: bool,
}

/// What a channel moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChannelPath {
    Translation,
    Rotation,
    Weights,
}

/// An animation's channels and what to tell about it.
#[derive(Debug, Clone)]
pub(crate) struct AnimationPlan {
    pub(crate) name: String,
    pub(crate) channels: Vec<ChannelPlan>,
    pub(crate) summary: AnimationSummary,
}

/// Plans the blocks of every channel of `animations` in `layout`, matching their tracks to the
/// bones of `targets`.
pub(crate) fn plan<'m>(
    layout: &mut Layout<'m>,
    animations: impl IntoIterator<Item = &'m Animation>,
    targets: &Targets<'_>,
) -> Vec<AnimationPlan> {
    animations
        .into_iter()
        .map(|animation| plan_animation(layout, animation, targets))
        .collect()
}

fn plan_animation<'m>(
    layout: &mut Layout<'m>,
    animation: &'m Animation,
    targets: &Targets<'_>,
) -> AnimationPlan {
    let bound = animation.bind(targets.bones);
    let mut skipped = bound.unbound;
    let mut moved: HashSet<usize> = HashSet::new();
    let mut channels = Vec::new();
    let mut keys = 0;
    let rate = animation.frames_per_second;
    for (bone, track) in bound.bound {
        let Some(node) = targets.node(bone).filter(|&node| moved.insert(node)) else {
            skipped.push(track.controller_id);
            continue;
        };
        if let Some(rotations) = &track.rotations {
            keys += rotations.frames.len();
            let times = layout.push(BlockData::Times(&rotations.frames, rate));
            let values = layout.push(BlockData::Rotations(&rotations.values));
            channels.push(channel(
                node,
                ChannelPath::Rotation,
                times,
                values,
                rotations,
            ));
        }
        if let Some(translations) = &track.translations {
            keys += translations.frames.len();
            let times = layout.push(BlockData::Times(&translations.frames, rate));
            let values = layout.push(BlockData::Translations(&translations.values));
            channels.push(channel(
                node,
                ChannelPath::Translation,
                times,
                values,
                translations,
            ));
        }
    }
    AnimationPlan {
        name: animation.name.clone(),
        summary: AnimationSummary {
            name: animation.name.clone(),
            duration_seconds: animation.duration_seconds(),
            additive: animation.additive,
            channels: channels.len(),
            keys,
            skipped_tracks: skipped,
            skipped_weights: 0,
        },
        channels,
    }
}

const fn channel<T>(
    node: usize,
    path: ChannelPath,
    times: usize,
    values: usize,
    keys: &Keys<T>,
) -> ChannelPlan {
    ChannelPlan {
        node,
        path,
        times,
        values,
        step: keys.frames.len() < 2,
    }
}

/// Writes the animations of `plans` that have channels.
pub(crate) fn push_animations(
    root: &mut Root,
    buffer: Index<json::Buffer>,
    layout: &Layout<'_>,
    plans: &[AnimationPlan],
) {
    for plan in plans.iter().filter(|plan| !plan.channels.is_empty()) {
        let mut samplers = Vec::with_capacity(plan.channels.len());
        let mut channels = Vec::with_capacity(plan.channels.len());
        for (position, channel) in plan.channels.iter().enumerate() {
            let (Some(times), Some(values)) = (
                layout.blocks.get(channel.times),
                layout.blocks.get(channel.values),
            ) else {
                continue;
            };
            let extent = time_extent(times.data);
            let input = push_accessor(
                root,
                buffer,
                times,
                extent
                    .as_ref()
                    .map(|(first, last)| (first.as_slice(), last.as_slice())),
            );
            let output = push_accessor(root, buffer, values, None);
            samplers.push(Sampler {
                extensions: None,
                extras: json::extras::Extras::default(),
                input,
                interpolation: Valid(if channel.step {
                    Interpolation::Step
                } else {
                    Interpolation::Linear
                }),
                output,
            });
            channels.push(Channel {
                sampler: Index::new(u32::try_from(position).unwrap_or(u32::MAX)),
                target: Target {
                    extensions: None,
                    extras: json::extras::Extras::default(),
                    node: Index::new(u32::try_from(channel.node).unwrap_or(u32::MAX)),
                    path: Valid(match channel.path {
                        ChannelPath::Rotation => Property::Rotation,
                        ChannelPath::Translation => Property::Translation,
                        ChannelPath::Weights => Property::MorphTargetWeights,
                    }),
                },
                extensions: None,
                extras: json::extras::Extras::default(),
            });
        }
        root.push(json::Animation {
            extensions: None,
            extras: json::extras::Extras::default(),
            channels,
            name: (!plan.name.is_empty()).then(|| plan.name.clone()),
            samplers,
        });
    }
}

/// Returns the first and last time of a block of key times as the accessor's `min` and `max`.
fn time_extent(data: BlockData<'_>) -> Option<(Vec<f32>, Vec<f32>)> {
    match data {
        BlockData::Times(frames, rate) => {
            let first = seconds(*frames.first()?, rate);
            let last = seconds(*frames.last()?, rate);
            Some((vec![first], vec![last]))
        }
        BlockData::Seconds(times) => Some((vec![*times.first()?], vec![*times.last()?])),
        _ => None,
    }
}
