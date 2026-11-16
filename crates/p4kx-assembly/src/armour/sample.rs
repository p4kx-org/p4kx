// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_character::{FacePose, FaceRig};
use p4kx_gltf::{SampledBone, SampledWeights};
use p4kx_model::{Bone, Track, multiply_quaternions};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use super::Armour;
use super::facial::FaceLink;

/// A joint offset, rotation component or weight under this is nothing: 0.1 micrometre for a
/// translation in metres, a few tenths of a micro-radian for a rotation.
const REST: f32 = 1e-7;

/// A control of the rig with the frames and values its track keys.
pub(super) type Control<'a> = (usize, &'a [u16], &'a [[f32; 3]]);

/// Evaluates the rig at each of `frames`, in parallel, for the controls' keyed values.
///
/// A control is a fraction, so its value is held to 0 to 1 whatever the file keys. Returns `None`
/// when a pose is not finite.
pub(super) fn evaluate(
    face: &FaceRig,
    controls: &[Control<'_>],
    frames: &[u16],
) -> Option<Vec<FacePose>> {
    let poses = evaluate_frames(face.logic().controls(), controls, frames, |values| {
        face.pose(values)
    });
    poses.iter().all(is_finite).then_some(poses)
}

pub(super) fn evaluate_frames<T: Send>(
    count: usize,
    controls: &[Control<'_>],
    frames: &[u16],
    pose: impl Fn(&[f32]) -> T + Sync,
) -> Vec<T> {
    frames
        .par_iter()
        .map(|&frame| {
            let mut values = vec![0.0_f32; count];
            for &(control, key_frames, key_values) in controls {
                if let Some(slot) = values.get_mut(control) {
                    *slot = value_at(key_frames, key_values, frame);
                }
            }
            pose(&values)
        })
        .collect()
}

pub(super) fn is_finite(pose: &FacePose) -> bool {
    pose.blendshapes
        .iter()
        .chain(pose.translations.iter().flatten())
        .chain(pose.rotations.iter().flatten())
        .all(|value| value.is_finite())
}

/// How far a sample may sit from the line between its neighbours before it is kept: 0.5 mm of a
/// joint translation, 0.001 per quaternion component (about 0.1 to 0.2 degrees), 0.01 of a weight.
const TRANSLATION_TOLERANCE: f32 = 5e-4;
const ROTATION_TOLERANCE: f32 = 1e-3;
const WEIGHT_TOLERANCE: f32 = 1e-2;

/// The most samples one line of the reduction spans, and the most samples reduced at once.
const STRETCH: usize = 128;
const CHUNK: usize = 1024;

/// The values of a pose, each with its tolerance; the reduction measures them against a straight
/// line, which viewers follow for translations and weights and, for rotations, approximately (they
/// use spherical interpolation, which differs negligibly between close rotations).
type Features = Vec<(f32, f32)>;

fn features(pose: &FacePose, link: &FaceLink) -> Features {
    let translations = pose
        .translations
        .iter()
        .flatten()
        .map(|&value| (value, TRANSLATION_TOLERANCE));
    let rotations = pose
        .rotations
        .iter()
        .flatten()
        .map(|&value| (value, ROTATION_TOLERANCE));
    let joints = translations.chain(rotations);
    let weights = link.blendshapes.iter().map(|&at| {
        (
            pose.blendshapes.get(at).copied().unwrap_or_default(),
            WEIGHT_TOLERANCE,
        )
    });
    joints.chain(weights).collect()
}

/// Returns the indices of the samples to keep: the first and the last, and every sample that a
/// viewer interpolating linearly between the kept ones would miss by more than the tolerance.
///
/// The samples are reduced a chunk at a time, and no line spans more than [`STRETCH`] samples, so
/// the work is linear in the samples and the chunk's values are all that is held.
pub(super) fn reduction(frames: &[u16], poses: &[FacePose], link: &FaceLink) -> Vec<usize> {
    let last = poses.len().saturating_sub(1);
    let mut kept = Vec::new();
    let mut start = 0;
    while start <= last && !poses.is_empty() {
        let end = (start + CHUNK).min(last);
        let values: Vec<Features> = poses
            .get(start..=end)
            .unwrap_or_default()
            .iter()
            .map(|pose| features(pose, link))
            .collect();
        let chunk_frames = frames.get(start..=end).unwrap_or_default();
        kept.extend(
            reduce(chunk_frames, &values)
                .into_iter()
                .map(|at| at + start),
        );
        if end == last {
            break;
        }
        start = end;
    }
    kept.dedup();
    kept
}

fn reduce(frames: &[u16], values: &[Features]) -> Vec<usize> {
    let mut kept = vec![0];
    let mut anchor = 0;
    let mut end = 1;
    while end < values.len() {
        let fits = (anchor + 1..end).all(|inner| line_fits((values, frames), (anchor, inner, end)));
        if !fits {
            anchor = end - 1;
            kept.push(anchor);
        } else if end - anchor >= STRETCH {
            anchor = end;
            kept.push(anchor);
            end += 1;
        } else {
            end += 1;
        }
    }
    if values.len() > 1 {
        kept.push(values.len() - 1);
    }
    kept.dedup();
    kept
}

fn line_fits(
    (values, frames): (&[Features], &[u16]),
    (from, at, to): (usize, usize, usize),
) -> bool {
    let frame = |index: usize| frames.get(index).copied().unwrap_or_default();
    let span = f32::from(frame(to) - frame(from));
    let t = f32::from(frame(at) - frame(from)) / span;
    match (values.get(from), values.get(at), values.get(to)) {
        (Some(start), Some(inner), Some(finish)) => {
            start
                .iter()
                .zip(inner)
                .zip(finish)
                .all(|((&(a, _), &(value, tolerance)), &(b, _))| {
                    (value - (b - a).mul_add(t, a)).abs() <= tolerance
                })
        }
        _ => true,
    }
}

/// The pose of a joint with the rest pose `rest` and the rotation `delta` from the rig: the delta
/// is multiplied on the right, so it turns in the joint's own axes.
pub(super) fn turned(rest: [f32; 4], delta: [f32; 4]) -> [f32; 4] {
    multiply_quaternions(rest, delta)
}

/// The poses of the facial joints that move, as the bones of the skeleton they are.
pub(super) fn joint_samples(
    bones: &[Bone],
    poses: &[FacePose],
    link: &FaceLink,
    evidence: (&[u32], &mut Vec<u32>),
) -> Vec<SampledBone> {
    joint_samples_with(bones, poses, link, evidence, turned)
}

pub(super) fn joint_samples_with(
    bones: &[Bone],
    poses: &[FacePose],
    link: &FaceLink,
    (hashes, skipped): (&[u32], &mut Vec<u32>),
    rotation: impl Fn([f32; 4], [f32; 4]) -> [f32; 4],
) -> Vec<SampledBone> {
    let mut out = Vec::new();
    for (joint, &bone) in link.joints.iter().enumerate() {
        let moves = poses.iter().any(|pose| {
            let [x, y, z] = pose.translations.get(joint).copied().unwrap_or_default();
            let [rx, ry, rz, _] = pose.rotations.get(joint).copied().unwrap_or_default();
            [x, y, z, rx, ry, rz].iter().any(|value| value.abs() > REST)
        });
        if !moves {
            continue;
        }
        let Some(rest) = bones.get(bone) else {
            skipped.extend(hashes.get(joint));
            continue;
        };
        let translations = poses
            .iter()
            .map(|pose| {
                let delta = pose.translations.get(joint).copied().unwrap_or_default();
                let mut at = rest.relative.translation;
                for (value, by) in at.iter_mut().zip(delta) {
                    *value += by;
                }
                at
            })
            .collect();
        let rotations = poses
            .iter()
            .map(|pose| {
                let delta = pose
                    .rotations
                    .get(joint)
                    .copied()
                    .unwrap_or([0.0, 0.0, 0.0, 1.0]);
                rotation(rest.relative.rotation, delta)
            })
            .collect();
        out.push(SampledBone::new(bone, translations, rotations));
    }
    out
}

/// Returns the times in seconds and the poses of the samples that the reduction keeps of the
/// `poses` at `frames`, at `rate` frames per second.
pub(super) fn reduced(
    (frames, poses): (&[u16], &[FacePose]),
    link: &FaceLink,
    rate: f32,
) -> (Vec<f32>, Vec<FacePose>) {
    let kept = reduction(frames, poses, link);
    let times = kept
        .iter()
        .filter_map(|&at| frames.get(at))
        .map(|&frame| f32::from(frame) / rate)
        .collect();
    let poses = kept
        .iter()
        .filter_map(|&at| poses.get(at))
        .cloned()
        .collect();
    (times, poses)
}

impl Armour {
    /// Returns the parts whose morph targets take weights, each with the blendshapes its targets
    /// stand for: the head part `head` with the link `link`, and the parts that follow its
    /// expressions.
    pub(super) fn weighed(&self, head: usize, link: &FaceLink) -> Vec<(usize, Vec<usize>)> {
        let followers = self
            .parts
            .iter()
            .enumerate()
            .filter(|(_, part)| !part.follows.is_empty() && part.follows.len() == part.morphs.len())
            .map(|(index, part)| {
                let numbers = part
                    .follows
                    .iter()
                    .filter_map(|&target| link.blendshapes.get(target).copied())
                    .collect();
                (index, numbers)
            });
        std::iter::once((head, link.blendshapes.clone()))
            .chain(followers)
            .collect()
    }
}

/// The weights of the morph targets of the part `part`, which stand for the blendshapes
/// `blendshapes`; none when no blendshape moves.
pub(super) fn weight_samples(
    poses: &[FacePose],
    blendshapes: &[usize],
    part: usize,
) -> Vec<SampledWeights> {
    let targets = blendshapes.len();
    if targets == 0 {
        return Vec::new();
    }
    let mut values = vec![0.0_f32; poses.len() * targets];
    let mut any = false;
    for (frame, pose) in poses.iter().enumerate() {
        for (target, &blendshape) in blendshapes.iter().enumerate() {
            let weight = pose
                .blendshapes
                .get(blendshape)
                .copied()
                .unwrap_or_default();
            if weight.abs() > REST
                && let Some(slot) = values.get_mut(frame * targets + target)
            {
                *slot = weight;
                any = true;
            }
        }
    }
    if any {
        vec![SampledWeights::new(part, values)]
    } else {
        Vec::new()
    }
}

/// Returns the frames and values of the translations of a track, which carry a control's value
/// in their first component.
pub(super) fn values_of(track: &Track) -> Option<(&[u16], &[[f32; 3]])> {
    let keys = track.translations.as_ref()?;
    Some((keys.frames.as_slice(), keys.values.as_slice()))
}

/// Returns the first component of the key values at `frame`, held to 0 to 1: interpolated between
/// the keys around it, held before the first and after the last. A value that is not finite is 0.
pub(super) fn value_at(frames: &[u16], values: &[[f32; 3]], frame: u16) -> f32 {
    let next = frames.partition_point(|&key| key <= frame);
    let at = |index: usize| {
        values.get(index).map_or(0.0, |value| {
            if value[0].is_finite() {
                value[0].clamp(0.0, 1.0)
            } else {
                0.0
            }
        })
    };
    let value = match (next.checked_sub(1), frames.get(next)) {
        (None, _) => at(0),
        (Some(last), None) => at(last),
        (Some(last), Some(&after)) => {
            let before = frames.get(last).copied().unwrap_or(frame);
            let span = f32::from(after - before);
            let t = f32::from(frame - before) / span;
            (at(next) - at(last)).mul_add(t, at(last))
        }
    };
    value.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::{Features, ROTATION_TOLERANCE, TRANSLATION_TOLERANCE, WEIGHT_TOLERANCE, reduce};

    fn three(tolerance: f32, middle: f32) -> Vec<Features> {
        [0.0, middle, 0.0]
            .into_iter()
            .map(|value| vec![(value, tolerance)])
            .collect()
    }

    fn kept(tolerance: f32, middle: f32) -> Vec<usize> {
        reduce(&[0, 1, 2], &three(tolerance, middle))
    }

    #[test]
    fn a_translation_half_a_millimetre_off_the_line_is_kept_and_less_is_dropped() {
        assert_eq!(kept(TRANSLATION_TOLERANCE, 0.0004), [0, 2]);
        assert_eq!(kept(TRANSLATION_TOLERANCE, 0.0006), [0, 1, 2]);
    }

    #[test]
    fn a_weight_is_kept_from_a_hundredth_off_the_line() {
        assert_eq!(kept(WEIGHT_TOLERANCE, 0.009), [0, 2]);
        assert_eq!(kept(WEIGHT_TOLERANCE, 0.011), [0, 1, 2]);
    }

    #[test]
    fn a_rotation_component_is_kept_from_a_thousandth_off_the_line() {
        assert_eq!(kept(ROTATION_TOLERANCE, 0.0009), [0, 2]);
        assert_eq!(kept(ROTATION_TOLERANCE, 0.0011), [0, 1, 2]);
    }
}
