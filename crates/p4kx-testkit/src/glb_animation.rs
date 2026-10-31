// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use gltf::accessor::{DataType, Dimensions};
use gltf::animation::{Interpolation, Property};
use gltf::{Accessor, Animation};

use crate::glb::CheckedGlb;

const UNIT_TOLERANCE: f32 = 0.000_5;

/// Checks the animations of `glb` against the rules of the Khronos validator a writer can break:
/// channels with distinct targets, strictly increasing key times with the first and last as
/// `min` and `max`, outputs of the type of their property with one value per key and unit
/// rotations.
pub(crate) fn check_animations(glb: &CheckedGlb) -> Result<(), String> {
    let nodes = glb.document.nodes().len();
    for animation in glb.document.animations() {
        check_animation(glb, &animation, nodes)?;
    }
    Ok(())
}

fn check_animation(
    glb: &CheckedGlb,
    animation: &Animation<'_>,
    nodes: usize,
) -> Result<(), String> {
    let name = format!("animation {}", animation.index());
    if animation.channels().next().is_none() {
        return Err(format!("{name} has no channels"));
    }
    let mut targets = HashSet::new();
    for channel in animation.channels() {
        let target = channel.target();
        let (node, property) = (target.node().index(), target.property());
        if node >= nodes {
            return Err(format!("{name}: channel targets node {node} of {nodes}"));
        }
        if !targets.insert((node, property as u8)) {
            return Err(format!(
                "{name}: two channels target {property:?} of node {node}"
            ));
        }
        let sampler = channel.sampler();
        let (input, output) = (sampler.input(), sampler.output());
        check_times(glb, &input, &name)?;
        let keys = input.count();
        let per_key = if property == Property::MorphTargetWeights {
            let targets = target
                .node()
                .mesh()
                .and_then(|mesh| mesh.primitives().next())
                .map_or(0, |primitive| primitive.morph_targets().count());
            if targets == 0 {
                return Err(format!(
                    "{name}: weights target a node without morph targets"
                ));
            }
            targets
        } else {
            1
        };
        let expected = match sampler.interpolation() {
            Interpolation::CubicSpline => 3 * keys * per_key,
            _ => keys * per_key,
        };
        if output.count() != expected {
            return Err(format!(
                "{name}: {} outputs for {keys} keys with {:?} interpolation",
                output.count(),
                sampler.interpolation()
            ));
        }
        check_output(glb, &output, property, &name)?;
    }
    Ok(())
}

fn check_times(glb: &CheckedGlb, input: &Accessor<'_>, name: &str) -> Result<(), String> {
    if input.dimensions() != Dimensions::Scalar || input.data_type() != DataType::F32 {
        return Err(format!("{name}: key times are no SCALAR of floats"));
    }
    let times = glb.floats(input);
    for (label, bound, expected) in [
        ("min", input.min(), times.first()),
        ("max", input.max(), times.last()),
    ] {
        let Some(bound) = bound else {
            return Err(format!("{name}: key times lack min or max"));
        };
        let found = match bound.as_array().map(Vec::as_slice) {
            Some([value]) => value.as_f64(),
            _ => None,
        };
        let matches = match (found, expected) {
            // serde_json without `float_roundtrip` may read a float one f64 step off, so the
            // bound is compared as the f32 it was written from.
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the writer stores the first and last time as f32 values"
            )]
            (Some(found), Some(expected)) => (found as f32).to_bits() == expected.to_bits(),
            _ => false,
        };
        if !matches {
            return Err(format!(
                "{name}: key times have {label} {bound}, not {expected:?}"
            ));
        }
    }
    if times.first().is_some_and(|time| *time < 0.0) {
        return Err(format!("{name}: a key time is negative"));
    }
    if times
        .windows(2)
        .any(|pair| matches!(pair, [a, b] if a >= b))
    {
        return Err(format!("{name}: key times do not increase strictly"));
    }
    Ok(())
}

fn check_output(
    glb: &CheckedGlb,
    output: &Accessor<'_>,
    property: Property,
    name: &str,
) -> Result<(), String> {
    let (dimensions, label) = match property {
        Property::Translation | Property::Scale => (Dimensions::Vec3, "VEC3"),
        Property::Rotation => (Dimensions::Vec4, "VEC4"),
        Property::MorphTargetWeights => {
            return if output.dimensions() == Dimensions::Scalar
                && output.data_type() == DataType::F32
                && glb.floats(output).iter().all(|weight| weight.is_finite())
            {
                Ok(())
            } else {
                Err(format!("{name}: weights are no finite SCALAR floats"))
            };
        }
    };
    if output.dimensions() != dimensions || output.data_type() != DataType::F32 {
        return Err(format!(
            "{name}: {property:?} outputs are no {label} of floats"
        ));
    }
    if property == Property::Rotation {
        for rotation in glb.floats(output).as_chunks::<4>().0 {
            let length = rotation
                .iter()
                .map(|value| value * value)
                .sum::<f32>()
                .sqrt();
            if (length - 1.0).abs() > UNIT_TOLERANCE {
                return Err(format!(
                    "{name}: rotation {rotation:?} is not of unit length"
                ));
            }
        }
    }
    Ok(())
}
