// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceUses, SourceValue, SourceValueState, SourcedValue};

use super::NominalBulletDamage;
use crate::CatalogError;
use crate::catalog_v3::authored_ammo_inputs::AuthoredAmmoInputs;
use crate::catalog_v3::authored_damage::DamageInfo;
use crate::catalog_v3::catalog_weapon_actions::ProjectileCombatInputs;
use crate::resource_float::ResourceFloat32;

const SECONDS_PER_MINUTE: f64 = 60.0;

pub(super) fn bullet_damage(
    sources: &SourceUses,
    action: &ProjectileCombatInputs<'_>,
    channels: &DamageInfo,
    selection: &[&FieldOrigin; 7],
) -> Result<NominalBulletDamage, CatalogError> {
    let [modes, slot, kind, route, index, projectile, payload] = *selection;
    let alpha = channels.channels().try_map(|damage| {
        let number = multiply(
            multiply(float(damage), positive_integer(action.pellet_count)),
            float(action.damage_multiplier),
        );
        let origins = [
            modes,
            slot,
            kind,
            route,
            index,
            projectile,
            payload,
            action.pellet_count.origin(),
            action.damage_multiplier.origin(),
            damage.origin(),
        ];
        sources.value_with_origins(number, &origins)
    })?;
    let rate = positive_integer(action.fire_rate);
    let nominal_dps = alpha.as_ref().try_map(|alpha| {
        let value = multiply(number(alpha), rate.clone());
        let number = match value {
            SourceValue::Present(value) => SourceValue::Present(value / SECONDS_PER_MINUTE),
            other => other,
        };
        sources.value_with_origins(number, &[alpha.origin(), action.fire_rate.origin()])
    })?;
    Ok(NominalBulletDamage { alpha, nominal_dps })
}

pub(super) fn flight_distance(
    sources: &SourceUses,
    ammo: &AuthoredAmmoInputs<'_>,
    origins: &[&FieldOrigin; 6],
) -> Result<SourcedValue<f64>, CatalogError> {
    let [modes, slot, kind, route, index, projectile] = *origins;
    let speed = ammo.speed();
    let lifetime = ammo.lifetime();
    Ok(sources.value_with_origins(
        multiply(float(speed), float(lifetime)),
        &[
            modes,
            slot,
            kind,
            route,
            index,
            projectile,
            speed.origin(),
            lifetime.origin(),
        ],
    )?)
}

fn multiply(left: SourceValue<f64>, right: SourceValue<f64>) -> SourceValue<f64> {
    match (left, right) {
        (SourceValue::Present(a), SourceValue::Present(b)) => SourceValue::Present(a * b),
        (SourceValue::Present(_), other) | (other, _) => other,
    }
}

fn float(cell: &SourcedValue<ResourceFloat32>) -> SourceValue<f64> {
    cell.value().map_or_else(
        || unavailable(cell.state()),
        |value| {
            value
                .finite
                .filter(|value| *value >= 0.0)
                .map_or(SourceValue::Invalid, |value| {
                    SourceValue::Present(f64::from(value))
                })
        },
    )
}

fn positive_integer(cell: &SourcedValue<i32>) -> SourceValue<f64> {
    match cell.value() {
        Some(&value) if value > 0 => SourceValue::Present(f64::from(value)),
        Some(_) => SourceValue::Invalid,
        None => unavailable(cell.state()),
    }
}

fn number(cell: &SourcedValue<f64>) -> SourceValue<f64> {
    cell.value()
        .copied()
        .map_or_else(|| unavailable(cell.state()), SourceValue::Present)
}

pub(super) const fn unavailable<T>(state: SourceValueState) -> SourceValue<T> {
    match state {
        SourceValueState::Absent => SourceValue::Absent,
        SourceValueState::Null => SourceValue::Null,
        SourceValueState::Unreadable => SourceValue::Unreadable,
        SourceValueState::Unsupported => SourceValue::Unsupported,
        _ => SourceValue::Invalid,
    }
}
