// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use serde::Serialize;

use super::combat_quality_math::{self, Adjustments, State};
use super::combat_quality_vectors::{AmmoStatus, Input, Part, Quality, Stat};

pub(super) type Channels = BTreeMap<String, f64>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PartResult {
    state: State,
    #[serde(skip_serializing_if = "Option::is_none")]
    projectile: Option<ProjectileResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shield: Option<Metric<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    beam: Option<BeamResult>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Metric<T> {
    pub(super) state: State,
    pub(super) value: Option<T>,
}

impl<T> Metric<T> {
    pub(super) const fn new(state: State, value: Option<T>) -> Self {
        Self { state, value }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectileResult {
    damage_per_projectile: Metric<Channels>,
    rpm: Metric<f64>,
    alpha: Metric<ChannelTotal>,
    nominal_dps: Metric<ChannelTotal>,
    travel_distance_metres: Metric<f64>,
    ammo_cost: u32,
}

#[derive(Clone, Serialize)]
pub(super) struct ChannelTotal {
    pub(super) channels: Channels,
    pub(super) total: f64,
}

impl ChannelTotal {
    pub(super) fn total(channels: Channels) -> Option<Self> {
        let total: f64 = channels.values().sum();
        total.is_finite().then_some(Self { channels, total })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BeamResult {
    authored_per_second: Channels,
    full_damage_range_metres: f64,
    zero_damage_range_metres: f64,
}

impl PartResult {
    const fn new(state: State) -> Self {
        Self {
            state,
            projectile: None,
            shield: None,
            beam: None,
        }
    }

    pub(super) fn alpha(&self) -> Option<&ChannelTotal> {
        self.projectile.as_ref()?.alpha.value.as_ref()
    }

    pub(super) fn dps(&self) -> Option<&ChannelTotal> {
        self.projectile.as_ref()?.nominal_dps.value.as_ref()
    }

    pub(super) fn health(&self) -> Option<f64> {
        self.shield.as_ref()?.value
    }
}

pub(super) fn calculate<'a>(
    part: &'a Part,
    crafted: Option<&'a [Quality]>,
    default: u32,
    properties: &'a BTreeMap<&'a str, Stat>,
) -> PartResult {
    let qualities = match crafted
        .map(|quality| combat_quality_math::resolve_qualities(part, quality))
        .transpose()
    {
        Ok(qualities) => qualities,
        Err(state) => return PartResult::new(state),
    };
    let inputs = Adjustments {
        part,
        qualities,
        defaults: default,
        properties,
    };
    match &part.input {
        Input::ShieldGenerator { max_health } => {
            let (state, value) = inputs.get(Stat::MaxHealth).apply(*max_health);
            let mut result = PartResult::new(if value.is_some() {
                State::Complete
            } else {
                State::Partial
            });
            result.shield = Some(Metric::new(state, value));
            result
        }
        Input::SimpleProjectile { .. } => projectile(&part.input, &inputs),
        Input::Beam {
            authored_per_second,
            full_damage_range,
            zero_damage_range,
        } => {
            let mut result = PartResult::new(State::UnsupportedAction);
            result.beam = Some(BeamResult {
                authored_per_second: authored_per_second.clone(),
                full_damage_range_metres: *full_damage_range,
                zero_damage_range_metres: *zero_damage_range,
            });
            result
        }
        Input::Charged { charge_seconds } => {
            assert!(
                charge_seconds.is_finite(),
                "fixture preserves a finite authored charge input"
            );
            PartResult::new(State::UnsupportedAction)
        }
    }
}

fn projectile(input: &Input, qualities: &Adjustments<'_>) -> PartResult {
    let Input::SimpleProjectile {
        ammo_status,
        ammo_cost,
        pellet_count,
        launcher_damage_multiplier,
        rpm,
        damage_per_projectile,
        speed_metres_per_second,
        lifetime_seconds,
    } = input
    else {
        return PartResult::new(State::UnsupportedAction);
    };
    if !matches!(ammo_status, AmmoStatus::Selected) {
        return PartResult::new(State::UnresolvedAmmo);
    }
    let damage = adjust_channels(damage_per_projectile.as_ref(), qualities);
    let (rate_state, rate) = qualities.get(Stat::Rpm).apply(*rpm);
    let rate = match rate {
        Some(value) if value > 0.0 => Metric::new(rate_state, Some(value)),
        Some(_) => Metric::new(State::InvalidTargetValue, None),
        None => Metric::new(rate_state, None),
    };
    let alpha = match launcher_damage_multiplier {
        Some(launch) if launch.is_finite() && *launch >= 0.0 && *pellet_count > 0 => {
            calculate_channels(damage.value.as_ref(), |value| {
                (value * f64::from(*pellet_count)) * launch
            })
        }
        Some(_) => Metric::new(State::InvalidTargetValue, None),
        None => Metric::new(State::MissingInput, None),
    };
    let dps = match rate.value {
        Some(rpm) => {
            calculate_channels(alpha.value.as_ref().map(|alpha| &alpha.channels), |value| {
                (value * rpm) / 60.0
            })
        }
        None => Metric::new(State::Unavailable, None),
    };
    let travel = travel_distance(*speed_metres_per_second, *lifetime_seconds);
    let all = alpha.value.is_some() && travel.value.is_some() && dps.value.is_some();
    let mut result = PartResult::new(if all { State::Complete } else { State::Partial });
    result.projectile = Some(ProjectileResult {
        damage_per_projectile: damage,
        rpm: rate,
        alpha,
        nominal_dps: dps,
        travel_distance_metres: travel,
        ammo_cost: *ammo_cost,
    });
    result
}

fn adjust_channels(channels: Option<&Channels>, qualities: &Adjustments<'_>) -> Metric<Channels> {
    let Some(channels) = channels.filter(|channels| !channels.is_empty()) else {
        return Metric::new(State::MissingInput, None);
    };
    let adjustment = qualities.get(Stat::DamagePerProjectile);
    let mut values = BTreeMap::new();
    let mut state = State::Unchanged;
    for (name, &base) in channels {
        let (result, value) = adjustment.apply(Some(base));
        let Some(value) = value else {
            return Metric::new(result, None);
        };
        state = result;
        values.insert(name.clone(), value);
    }
    Metric::new(state, Some(values))
}

fn calculate_channels(
    channels: Option<&Channels>,
    apply: impl Fn(f64) -> f64,
) -> Metric<ChannelTotal> {
    let Some(channels) = channels else {
        return Metric::new(State::Unavailable, None);
    };
    let scaled: Channels = channels
        .iter()
        .map(|(name, &base)| (name.clone(), apply(base)))
        .collect();
    if scaled
        .values()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Metric::new(State::InvalidTargetValue, None);
    }
    ChannelTotal::total(scaled).map_or_else(
        || Metric::new(State::InvalidTargetValue, None),
        |total| Metric::new(State::Complete, Some(total)),
    )
}

fn travel_distance(speed: Option<f64>, lifetime: Option<f64>) -> Metric<f64> {
    let Some((speed, lifetime)) = speed.zip(lifetime) else {
        return Metric::new(State::MissingInput, None);
    };
    let value = speed * lifetime;
    if speed < 0.0 || lifetime < 0.0 || !value.is_finite() {
        Metric::new(State::InvalidTargetValue, None)
    } else {
        Metric::new(State::Complete, Some(value))
    }
}
