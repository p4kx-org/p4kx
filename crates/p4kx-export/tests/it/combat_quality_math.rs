// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use num_bigint::BigInt;
use num_traits::{FromPrimitive, ToPrimitive};

use super::combat_quality_vectors::{
    Kind, Modifier, Part, Quality, RangeType, Segment, Stat, Step,
};

const MAX_QUALITY: f64 = 1_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum State {
    Factory,
    Adjusted,
    Unchanged,
    InvalidQuality,
    UnknownSlot,
    DuplicateSlot,
    UnresolvedProperty,
    UnsupportedRange,
    UnsupportedOverlap,
    UnsupportedNumeric,
    InvalidTargetValue,
    MissingInput,
    UnresolvedAmmo,
    UnsupportedAction,
    Complete,
    Partial,
    Unavailable,
}

pub(super) enum Adjustment {
    Factory,
    Unchanged,
    Factor(f64),
    Summand(f64),
    Unsupported(State),
}

impl Adjustment {
    pub(super) fn apply(&self, base: Option<f64>) -> (State, Option<f64>) {
        let (state, value) = match self {
            Self::Factory => (State::Factory, base),
            Self::Unchanged => (State::Unchanged, base),
            Self::Factor(factor) => (State::Adjusted, base.map(|base| base * factor)),
            Self::Summand(summand) => (State::Adjusted, base.map(|base| base + summand)),
            Self::Unsupported(state) => return (*state, None),
        };
        let Some(value) = value else {
            return (State::MissingInput, None);
        };
        if value.is_finite() && value >= 0.0 {
            (state, Some(value))
        } else {
            (State::InvalidTargetValue, None)
        }
    }
}

type Qualities<'a> = BTreeMap<&'a [Step], f64>;

pub(super) fn resolve_qualities<'a>(
    part: &'a Part,
    selected: &'a [Quality],
) -> Result<Qualities<'a>, State> {
    let mut quality_by_path = BTreeMap::new();
    for quality in selected {
        let value = quality.score;
        if value.fract() != 0.0 || !(0.0..=MAX_QUALITY).contains(&value) {
            return Err(State::InvalidQuality);
        }
        let path = quality.slot_path.as_slice();
        if quality.blueprint_guid != part.blueprint_guid
            || quality.tier_index != part.tier_index
            || !part.slots.iter().any(|slot| slot.slot_path == path)
        {
            return Err(State::UnknownSlot);
        }
        if quality_by_path.insert(path, value).is_some() {
            return Err(State::DuplicateSlot);
        }
    }
    Ok(quality_by_path)
}

pub(super) struct Adjustments<'a> {
    pub(super) part: &'a Part,
    pub(super) qualities: Option<Qualities<'a>>,
    pub(super) defaults: u32,
    pub(super) properties: &'a BTreeMap<&'a str, Stat>,
}

impl Adjustments<'_> {
    pub(super) fn get(&self, stat: Stat) -> Adjustment {
        let Some(qualities) = &self.qualities else {
            return Adjustment::Factory;
        };
        let mut inputs = self.part.slots.iter().flat_map(|slot| {
            let quality = qualities
                .get(slot.slot_path.as_slice())
                .copied()
                .unwrap_or_else(|| f64::from(self.defaults));
            slot.modifiers
                .iter()
                .map(move |modifier| (quality, modifier))
        });
        let adjustment = inputs.try_fold(None, |found, (quality, modifier)| {
            let target = self
                .properties
                .get(modifier.property_id.as_str())
                .ok_or(State::UnresolvedProperty)?;
            if *target != stat {
                return Ok(found);
            }
            Contributions::combine(found, quality, modifier).map(Some)
        });
        match adjustment {
            Ok(None) => Adjustment::Unchanged,
            Ok(Some(found)) => found.finish(),
            Err(error) => Adjustment::Unsupported(error),
        }
    }
}

enum Sum {
    Factor(f64),
    IntegerSum(BigInt),
}

struct Contributions<'property> {
    property: &'property str,
    sum: Sum,
}

impl<'property> Contributions<'property> {
    fn combine(
        previous: Option<Self>,
        quality: f64,
        modifier: &'property Modifier,
    ) -> Result<Self, State> {
        let property = modifier.property_id.as_str();
        let initial = match modifier.value_kind {
            Kind::Multiplier => Sum::Factor(1.0),
            Kind::Additive => Sum::IntegerSum(BigInt::default()),
        };
        let mut total = previous.unwrap_or(Self {
            property,
            sum: initial,
        });
        if total.property != property {
            return Err(State::UnsupportedOverlap);
        }
        let contribution = curve(modifier, quality).ok_or(State::UnsupportedRange)?;
        match (&mut total.sum, modifier.value_kind) {
            (Sum::Factor(result), Kind::Multiplier) => *result += contribution - 1.0,
            (Sum::IntegerSum(result), Kind::Additive) => {
                *result += BigInt::from_f64(contribution.round_ties_even())
                    .ok_or(State::UnsupportedNumeric)?;
            }
            _ => return Err(State::UnsupportedOverlap),
        }
        Ok(total)
    }

    fn finish(self) -> Adjustment {
        match self.sum {
            Sum::Factor(value) if value.is_finite() => Adjustment::Factor(value),
            Sum::IntegerSum(value) => value
                .to_f64()
                .filter(|number| BigInt::from_f64(*number).as_ref() == Some(&value))
                .map_or(
                    Adjustment::Unsupported(State::UnsupportedNumeric),
                    Adjustment::Summand,
                ),
            Sum::Factor(_) => Adjustment::Unsupported(State::UnsupportedNumeric),
        }
    }
}

fn curve(modifier: &Modifier, quality: f64) -> Option<f64> {
    if modifier.segments.is_empty() {
        return interpolate(modifier.band.as_ref()?, modifier.value_kind, quality);
    }
    let last = modifier.segments.len() - 1;
    let (_, segment) = modifier
        .segments
        .iter()
        .enumerate()
        .find(|(index, segment)| {
            segment.a.is_some()
                && segment
                    .b
                    .is_some_and(|end| quality <= quality_bound(end) || *index == last)
        })?;
    let candidate = match modifier.range_type {
        RangeType::Step => source_number(segment.start.or(segment.end)?, modifier.value_kind)?,
        RangeType::Linear => interpolate(segment, modifier.value_kind, quality)?,
    };
    candidate.is_finite().then_some(candidate)
}

#[expect(
    clippy::suboptimal_flops,
    reason = "the shared formula uses separate binary64 operations"
)]
fn interpolate(segment: &Segment, kind: Kind, quality: f64) -> Option<f64> {
    let (a, b) = (quality_bound(segment.a?), quality_bound(segment.b?));
    let (start, end) = (
        source_number(segment.start?, kind)?,
        source_number(segment.end?, kind)?,
    );
    let t = if segment.a == segment.b {
        0.0
    } else {
        ((quality - a) / (b - a)).clamp(0.0, 1.0)
    };
    let candidate = start + (end - start) * t;
    candidate.is_finite().then_some(candidate)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "the client formula operates on binary64 quality bounds"
)]
const fn quality_bound(value: i64) -> f64 {
    value as f64
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "multiplier endpoints are authored binary32"
)]
fn source_number(input: f64, kind: Kind) -> Option<f64> {
    let decoded = match kind {
        Kind::Multiplier => f64::from(input as f32),
        Kind::Additive => input,
    };
    decoded.is_finite().then_some(decoded)
}
