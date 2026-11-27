// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[cfg(test)]
use std::path::Path;

use p4kx_extraction::{CancelToken, DecimalU64, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::component::{Component, ComponentKey};
use super::number::Number;
use crate::CatalogError;
#[cfg(test)]
use crate::ExportAccountingError;
use crate::catalog_queries::equipment::{DpsUnit, FireRateUnit, WeaponActions};
#[cfg(test)]
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_query_session::checkpoint;
use crate::catalog_rows::staged::StagedCatalogRows;
#[cfg(test)]
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::catalog_v3::catalog_weapon_actions::{BeamDamageInputs, WeaponActionKind};
#[cfg(test)]
use crate::item_stats::WEAPON_COMPONENT;
use crate::resource_float::ResourceFloat32;

#[cfg(test)]
#[path = "weapon_actions_tests.rs"]
mod tests;

pub(super) const PATH: &str = "configurations.weaponActions";

/// Stages the selected weapon fields using the per-owner canonical visitor.
#[cfg(test)]
pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<StagedCatalogTable, ReducedError> {
    let mut staged = StagedCatalogRows::new(scratch, cancel);
    let mut observed = 0_usize;
    let summary = parsed.visit_weapon_sources(
        CatalogRecordSelection::All,
        cancel,
        |index, snapshot| -> Result<(), ReducedError> {
            for owner in snapshot.rows() {
                for source in &owner.weapon_actions {
                    append(&mut staged, index, source, cancel)?;
                }
                observed = observed
                    .checked_add(owner.weapon_actions.len())
                    .ok_or_else(|| CatalogError::from(ExportAccountingError::CounterOverflow))?;
            }
            Ok(())
        },
    )?;
    let table = parsed.project_fallible(cancel, |_, _, forge| {
        Ok::<_, ReducedError>(
            staged.finish_matching_source_types(forge, |name| name == WEAPON_COMPONENT)?,
        )
    })?;
    completeness::inventory(PATH, table.availability(), summary.coverage())?;
    completeness::rows(PATH, observed, table.len())?;
    Ok(table)
}

/// Stages one physical weapon without reading its Item again.
pub(super) fn append(
    staged: &mut StagedCatalogRows<'_>,
    owner_index: u32,
    source: &WeaponActions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<(), ReducedError> {
    if ComponentKey::read(source.owner(), PATH)?.record != owner_index {
        return Err(incomplete());
    }
    let originals = source.action_nodes();
    let mut action_nodes = Vec::new();
    action_nodes
        .try_reserve_exact(originals.len())
        .map_err(CatalogError::SourceUseAllocation)?;
    for action in originals {
        checkpoint(cancel)?;
        let position = completeness::cell(PATH, action.source_ordinal())?;
        if position.value.is_none() {
            return Err(incomplete());
        }
        action_nodes.push(Action {
            source_type: text(action.source_type())?,
            source_value_ordinal: position,
            kind: Kind::read(action.kind(), source)?,
        });
    }
    staged.append(&Row {
        owner: source.owner().into(),
        fire_actions: links(source, source.fire_actions())?,
        aim_action: link(source, source.aim_action())?,
        action_nodes,
    })?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row<'read> {
    #[serde(flatten)]
    owner: Component<'read>,
    fire_actions: Cell<Vec<Cell<u32>>>,
    aim_action: Cell<u32>,
    action_nodes: Vec<Action<'read>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Action<'read> {
    source_type: Cell<&'read str>,
    source_value_ordinal: Cell<&'read DecimalU64>,
    kind: Kind<'read>,
}

#[derive(Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Kind<'read> {
    Single(Rate<'read>),
    Rapid(Rate<'read>),
    Burst(Rate<'read>),
    Beam(Beam<'read>),
    TractorBeam(Box<Tractor>),
    Aim,
    Sequence {
        mode: Cell<&'read str>,
        sequence_entries: Cell<Vec<Cell<Child>>>,
    },
    Parallel {
        weapon_actions: Cell<Vec<Cell<u32>>>,
    },
    Conditional(Condition),
    DynamicConditional(Condition),
    Toggle(Child),
    Charged(Child),
    Authored,
}

impl<'read> Kind<'read> {
    fn read(
        kind: &'read WeaponActionKind<'_>,
        weapon: &WeaponActions<'_>,
    ) -> Result<Self, ReducedError> {
        Ok(match kind {
            WeaponActionKind::Single(_) => Self::Single(Rate::read(kind)?),
            WeaponActionKind::Rapid(_) => Self::Rapid(Rate::read(kind)?),
            WeaponActionKind::Burst(_) => Self::Burst(Rate::read(kind)?),
            WeaponActionKind::Beam(beam) => Self::Beam(Beam {
                hit_type: text(beam.hit_type())?,
                damage_per_second: completeness::cell(PATH, beam.damage_per_second())?.try_map(
                    |damage| -> Result<_, ReducedError> {
                        Ok(BeamDamage {
                            source_type: text(damage.source_type())?,
                            inputs: DamageInputs::read(damage.inputs())?,
                        })
                    },
                )?,
                damage_per_second_unit: DpsUnit::DamagePointsPerSecond,
            }),
            WeaponActionKind::TractorBeam(beam) => Self::TractorBeam(Box::new(Tractor {
                min_force: quantity(beam.min_force())?,
                max_force: quantity(beam.max_force())?,
                min_distance: quantity(beam.min_distance())?,
                max_distance: quantity(beam.max_distance())?,
                full_strength_distance: quantity(beam.full_strength_distance())?,
                max_volume: quantity(beam.max_volume())?,
            })),
            WeaponActionKind::Aim(_) => Self::Aim,
            WeaponActionKind::Sequence(sequence) => Self::Sequence {
                mode: text(sequence.mode())?,
                sequence_entries: completeness::list(PATH, sequence.sequence_entries(), |entry| {
                    Child::read(weapon, entry.weapon_action())
                })?,
            },
            WeaponActionKind::Parallel(parallel) => Self::Parallel {
                weapon_actions: links(weapon, parallel.weapon_actions())?,
            },
            WeaponActionKind::Conditional(condition)
            | WeaponActionKind::DynamicConditional(condition) => {
                let condition = Condition {
                    conditional_weapon_actions: completeness::list(
                        PATH,
                        condition.conditional_weapon_actions(),
                        |entry| Child::read(weapon, entry.weapon_action()),
                    )?,
                    default_weapon_action: link(weapon, condition.default_weapon_action())?,
                };
                if matches!(kind, WeaponActionKind::Conditional(_)) {
                    Self::Conditional(condition)
                } else {
                    Self::DynamicConditional(condition)
                }
            }
            WeaponActionKind::Toggle(toggle) => {
                Self::Toggle(Child::read(weapon, toggle.weapon_action())?)
            }
            WeaponActionKind::Charged(charge) => {
                Self::Charged(Child::read(weapon, charge.weapon_action())?)
            }
            WeaponActionKind::Authored(_) => Self::Authored,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Rate<'read> {
    fire_rate: Cell<&'read i32>,
    fire_rate_unit: FireRateUnit,
}

impl<'read> Rate<'read> {
    fn read(action: &'read WeaponActionKind<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            fire_rate: completeness::cell(PATH, action.fire_rate().ok_or_else(incomplete)?)?,
            fire_rate_unit: FireRateUnit::DischargesPerMinute,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Child {
    weapon_action: Cell<u32>,
}

impl Child {
    fn read(weapon: &WeaponActions<'_>, slot: &SourcedValue<u32>) -> Result<Self, ReducedError> {
        Ok(Self {
            weapon_action: link(weapon, slot)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Condition {
    conditional_weapon_actions: Cell<Vec<Cell<Child>>>,
    default_weapon_action: Cell<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Beam<'read> {
    hit_type: Cell<&'read str>,
    damage_per_second: Cell<BeamDamage<'read>>,
    damage_per_second_unit: DpsUnit,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BeamDamage<'read> {
    source_type: Cell<&'read str>,
    #[serde(flatten)]
    inputs: DamageInputs,
}

#[derive(Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum DamageInputs {
    Base,
    Channels(Box<Channels>),
    Parameters { damage_total: Quantity },
    Authored,
}

impl DamageInputs {
    fn read(source: &BeamDamageInputs<'_>) -> Result<Self, ReducedError> {
        Ok(match source {
            BeamDamageInputs::Base => Self::Base,
            BeamDamageInputs::Channels(channels) => {
                let channels = channels.channels();
                Self::Channels(Box::new(Channels {
                    physical: quantity(channels.physical())?,
                    energy: quantity(channels.energy())?,
                    distortion: quantity(channels.distortion())?,
                    thermal: quantity(channels.thermal())?,
                    biochemical: quantity(channels.biochemical())?,
                    stun: quantity(channels.stun())?,
                }))
            }
            BeamDamageInputs::Parameters(parameters) => Self::Parameters {
                damage_total: quantity(parameters.damage_total())?,
            },
            BeamDamageInputs::Authored => Self::Authored,
        })
    }
}

type Quantity = Cell<Number<'static>>;

#[derive(Serialize)]
struct Channels {
    physical: Quantity,
    energy: Quantity,
    distortion: Quantity,
    thermal: Quantity,
    biochemical: Quantity,
    stun: Quantity,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Tractor {
    min_force: Quantity,
    max_force: Quantity,
    min_distance: Quantity,
    max_distance: Quantity,
    full_strength_distance: Quantity,
    max_volume: Quantity,
}

fn text<'read>(value: &'read SourcedValue<&str>) -> Result<Cell<&'read str>, ReducedError> {
    Ok(completeness::cell(PATH, value)?.map(|value| *value))
}

fn quantity(value: &SourcedValue<ResourceFloat32>) -> Result<Quantity, ReducedError> {
    Ok(completeness::cell(PATH, value)?.map(Number::from))
}

fn link(weapon: &WeaponActions<'_>, slot: &SourcedValue<u32>) -> Result<Cell<u32>, ReducedError> {
    completeness::cell(PATH, slot)?.try_map(|target| verify(weapon, *target))
}

fn links(
    weapon: &WeaponActions<'_>,
    slots: &SourcedValue<Vec<SourcedValue<u32>>>,
) -> Result<Cell<Vec<Cell<u32>>>, ReducedError> {
    completeness::list(PATH, slots, |target| verify(weapon, *target))
}

fn verify(weapon: &WeaponActions<'_>, target: u32) -> Result<u32, ReducedError> {
    weapon.mode(target).map(|_| target).ok_or_else(incomplete)
}

const fn incomplete() -> ReducedError {
    ReducedError::IncompleteScope(PATH)
}
