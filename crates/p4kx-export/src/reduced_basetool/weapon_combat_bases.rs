// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DecimalU64, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::component::{Component, ComponentKey};
use super::number::{Float, Number};
use super::record_targets::RecordTargets;
use crate::catalog_queries::equipment::{
    CombatModel, CombatUnits, DamageChannels, NominalBullet, NominalBulletDamage, WeaponCombatBase,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_rows::staged::StagedCatalogRows;

pub(super) const PATH: &str = "projections.weaponCombatBases";

pub(super) fn append(
    output: &mut StagedCatalogRows<'_>,
    owner_index: u32,
    rows: &[WeaponCombatBase<'_>],
    items: &RecordTargets,
    ammunition: &RecordTargets,
    cancel: Option<&CancelToken>,
) -> Result<(), ReducedError> {
    for row in rows {
        checkpoint(cancel)?;
        let physical = ComponentKey::read(row.owner(), PATH)?;
        if physical.record != owner_index {
            return Err(ReducedError::IncompleteScope(PATH));
        }
        items.verify(PATH, physical.record, physical.guid.get())?;
        let output_row = Row::read(row, ammunition)?;
        output.append(&output_row)?;
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row<'read> {
    #[serde(flatten)]
    owner: Component<'read>,
    formula_version: u8,
    units: &'read CombatUnits,
    mode_index: Cell<&'read u32>,
    action_source_value_ordinal: Option<Cell<&'read DecimalU64>>,
    action_source_type: Option<Cell<&'read str>>,
    ammunition_record_index: Option<Cell<&'read u32>>,
    model: Cell<Model<'read>>,
}

impl<'read> Row<'read> {
    fn read(
        row: &'read WeaponCombatBase<'_>,
        ammunition: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        let ammunition_record_index = optional(row.ammunition_record_index())?;
        if let Some(index) = ammunition_record_index.as_ref().and_then(|cell| cell.value) {
            ammunition.verify_index(PATH, *index)?;
        }
        Ok(Self {
            owner: row.owner().into(),
            formula_version: row.formula_version(),
            units: row.units(),
            mode_index: completeness::cell(PATH, row.mode_index())?,
            action_source_value_ordinal: optional(row.action_source_value_ordinal())?,
            action_source_type: optional(row.action_source_type())?
                .map(|cell| cell.map(|name| *name)),
            ammunition_record_index,
            model: completeness::cell(PATH, row.model())?.try_map(Model::read)?,
        })
    }
}

fn optional<T>(source: Option<&SourcedValue<T>>) -> Result<Option<Cell<&T>>, ReducedError> {
    source
        .map(|source| completeness::cell(PATH, source))
        .transpose()
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Model<'read> {
    NominalBullet {
        values: Box<Bullet<'read>>,
    },
    #[serde(untagged)]
    Outcome(&'read CombatModel),
}

impl<'read> Model<'read> {
    fn read(source: &'read CombatModel) -> Result<Self, ReducedError> {
        match source {
            CombatModel::NominalBullet { values } => Ok(Self::NominalBullet {
                values: Box::new(Bullet::read(values)?),
            }),
            CombatModel::AmmoSelectionUnavailable
            | CombatModel::AmmoDefinitionUnavailable
            | CombatModel::ActionUnavailable
            | CombatModel::UnsupportedAction
            | CombatModel::Burst
            | CombatModel::Beam
            | CombatModel::Charge
            | CombatModel::Conditional
            | CombatModel::ProjectileUnavailable
            | CombatModel::UnsupportedProjectile => Ok(Self::Outcome(source)),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Bullet<'read> {
    fire_rate_rpm: Cell<&'read i32>,
    projectiles_per_discharge: Cell<&'read i32>,
    launcher_damage_multiplier: Cell<Number<'read>>,
    nominal_flight_distance: Cell<Float<f64>>,
    damage: Cell<Damage>,
}

impl<'read> Bullet<'read> {
    fn read(bullet: &'read NominalBullet) -> Result<Self, ReducedError> {
        Ok(Self {
            fire_rate_rpm: completeness::cell(PATH, bullet.fire_rate_rpm())?,
            projectiles_per_discharge: completeness::cell(
                PATH,
                bullet.projectiles_per_discharge(),
            )?,
            launcher_damage_multiplier: completeness::cell(
                PATH,
                bullet.launcher_damage_multiplier(),
            )?
            .map(Number::from),
            nominal_flight_distance: number(bullet.nominal_flight_distance())?,
            damage: completeness::cell(PATH, bullet.damage())?.try_map(Damage::read)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Damage {
    alpha: DamageChannels<Cell<Float<f64>>>,
    nominal_dps: DamageChannels<Cell<Float<f64>>>,
}

impl Damage {
    fn read(source: &NominalBulletDamage) -> Result<Self, ReducedError> {
        Ok(Self {
            alpha: source.alpha().as_ref().try_map(number)?,
            nominal_dps: source.nominal_dps().as_ref().try_map(number)?,
        })
    }
}

fn number(value: &SourcedValue<f64>) -> Result<Cell<Float<f64>>, ReducedError> {
    Ok(completeness::cell(PATH, value)?.map(|value| Float::from(*value)))
}
