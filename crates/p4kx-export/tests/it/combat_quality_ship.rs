// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::combat_quality_math::State;
use super::combat_quality_parity::TestResult;
use super::combat_quality_part::{self, ChannelTotal, Channels, Metric};
use super::combat_quality_vectors::{Case, Group, Input, Placement, Stat};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Ship {
    groups: BTreeMap<String, Weapons>,
    max_health: Metric<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Weapons {
    alpha: Metric<ChannelTotal>,
    nominal_dps: Metric<ChannelTotal>,
}

#[derive(Default)]
struct GroupInputs {
    alpha: ChannelSum,
    dps: ChannelSum,
}

pub(super) fn calculate(
    case: &Case,
    placements: &[Placement],
    crafted: bool,
    default: u32,
    mapping: &BTreeMap<&str, Stat>,
) -> TestResult<Ship> {
    let mut seen = BTreeSet::new();
    let mut groups = BTreeMap::<&Group, GroupInputs>::new();
    let mut shields = ValueSum::<f64>::default();
    for placement in placements {
        assert!(
            !placement.port_path.is_empty() && seen.insert(&placement.port_path),
            "each installed placement has a unique physical port path",
        );
        let part = case
            .parts
            .iter()
            .find(|part| part.id == placement.part_id)
            .ok_or("selected placement must reference a fixture part")?;
        let qualities = crafted.then(|| placement.qualities.as_deref().unwrap_or(&part.qualities));
        let result = combat_quality_part::calculate(part, qualities, default, mapping);
        if matches!(part.input, Input::ShieldGenerator { .. }) {
            assert!(
                placement.group.is_none(),
                "shield totals are independent of the firing group"
            );
            shields.add(result.health(), |left, right| *left += *right);
        } else {
            let group = placement
                .group
                .as_ref()
                .ok_or("each weapon placement names its firing group")?;
            let target = groups.entry(group).or_default();
            target.alpha.add(result.alpha());
            target.dps.add(result.dps());
        }
    }
    let groups = groups
        .into_iter()
        .map(|(group, inputs)| {
            (
                group.key(),
                Weapons {
                    alpha: inputs.alpha.finish(),
                    nominal_dps: inputs.dps.finish(),
                },
            )
        })
        .collect();
    Ok(Ship {
        groups,
        max_health: shields.finish(),
    })
}

#[derive(Default)]
struct ValueSum<T> {
    value: Option<T>,
    missing: bool,
}

impl<T: Clone> ValueSum<T> {
    fn add(&mut self, value: Option<T>, sum: impl FnOnce(&mut T, &T)) {
        if let Some(value) = value {
            if let Some(current) = self.value.as_mut() {
                sum(current, &value);
            } else {
                self.value = Some(value);
            }
        } else {
            self.missing = true;
        }
    }

    fn finish(self) -> Metric<T> {
        let state = match (&self.value, self.missing) {
            (Some(_), false) => State::Complete,
            (Some(_), true) => State::Partial,
            (None, _) => State::Unavailable,
        };
        Metric::new(state, self.value)
    }
}

#[derive(Default)]
struct ChannelSum(ValueSum<Channels>);

impl ChannelSum {
    fn add(&mut self, value: Option<&ChannelTotal>) {
        self.0.add(
            value.map(|value| value.channels.clone()),
            |existing, contribution| {
                for (channel, amount) in contribution {
                    *existing.entry(channel.clone()).or_default() += amount;
                }
            },
        );
    }

    fn finish(self) -> Metric<ChannelTotal> {
        let result = self.0.finish();
        let value = result.value.and_then(ChannelTotal::total);
        Metric::new(
            if value.is_some() {
                result.state
            } else {
                State::Unavailable
            },
            value,
        )
    }
}
