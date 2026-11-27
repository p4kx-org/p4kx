// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::ammo_definitions;
use super::cell::Cell;
use super::completeness;
use super::component::{Component, ComponentKey};
use super::item_definitions;
use super::record_targets::RecordTargets;
use crate::catalog_queries::equipment::{
    ProjectileAmmoChoice, ProjectileAmmoRole, WeaponProjectileAmmoSelection,
};
use crate::catalog_rows::staged::StagedCatalogRows;

pub(super) const PATH: &str = "projections.weaponProjectileAmmoSelections";

pub(super) struct Targets<'read> {
    pub(super) ammunition: &'read RecordTargets,
    pub(super) items: &'read RecordTargets,
}

pub(super) fn append(
    output: &mut StagedCatalogRows<'_>,
    rows: &[WeaponProjectileAmmoSelection],
    targets: &Targets<'_>,
) -> Result<(), ReducedError> {
    for original in rows {
        output.append(&Row::read(original, targets)?)?;
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row<'read> {
    #[serde(flatten)]
    owner: Component<'read>,
    action_source_value_ordinal: Cell<&'read DecimalU64>,
    selection: Cell<Outcome<'read>>,
}

impl<'read> Row<'read> {
    fn read(
        original: &'read WeaponProjectileAmmoSelection,
        targets: &Targets<'_>,
    ) -> Result<Self, ReducedError> {
        let owner = original.owner();
        let key = ComponentKey::read(owner, PATH)?;
        targets
            .items
            .verify(item_definitions::PATH, key.record, key.guid.get())?;
        Ok(Self {
            owner: owner.into(),
            action_source_value_ordinal: required(original.action_source_value_ordinal())?,
            selection: Cell::from(original.selection())
                .try_map(|choice| Outcome::read(choice, targets))?,
        })
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum Outcome<'read> {
    Resolved(Route<'read>),
    Rejected(&'read ProjectileAmmoChoice),
}

#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Route<'read> {
    Selected {
        role: Cell<&'read ProjectileAmmoRole>,
        container_component_ordinal: Cell<&'read u32>,
        ammo_record_index: Cell<&'read u32>,
    },
    SelectedLinkedMagazine {
        role: Cell<&'read ProjectileAmmoRole>,
        magazine_record_index: Cell<&'read u32>,
        container_component_ordinal: Cell<&'read u32>,
        ammo_record_index: Cell<&'read u32>,
    },
}

impl<'read> Outcome<'read> {
    fn read(
        original: &'read ProjectileAmmoChoice,
        targets: &Targets<'_>,
    ) -> Result<Self, ReducedError> {
        use ProjectileAmmoChoice as Choice;

        match original {
            Choice::Selected {
                role,
                container_component_ordinal,
                ammo_record_index,
            } => Ok(Self::Resolved(Route::Selected {
                role: required(role)?,
                container_component_ordinal: required(container_component_ordinal)?,
                ammo_record_index: index(
                    ammo_definitions::PATH,
                    targets.ammunition,
                    ammo_record_index,
                )?,
            })),
            Choice::SelectedLinkedMagazine {
                role,
                magazine_record_index,
                container_component_ordinal,
                ammo_record_index,
            } => Ok(Self::Resolved(Route::SelectedLinkedMagazine {
                role: required(role)?,
                magazine_record_index: index(
                    item_definitions::PATH,
                    targets.items,
                    magazine_record_index,
                )?,
                container_component_ordinal: required(container_component_ordinal)?,
                ammo_record_index: index(
                    ammo_definitions::PATH,
                    targets.ammunition,
                    ammo_record_index,
                )?,
            })),
            rejected @ (Choice::DanglingMagazine
            | Choice::UnsupportedMagazineTarget
            | Choice::SelfLinkedMagazine
            | Choice::AmbiguousMagazineReference
            | Choice::UnreadableMagazine
            | Choice::LinkedMagazineWithoutContainer
            | Choice::AmbiguousLinkedMagazineContainer
            | Choice::IncompleteLinkedMagazineComponents
            | Choice::AmbiguousAction
            | Choice::IncompleteActions
            | Choice::AmbiguousComponents
            | Choice::IncompleteComponents
            | Choice::NoAdmittedOwnContainer
            | Choice::OtherMagazineRoute
            | Choice::IndeterminateMagazineRoute
            | Choice::UnsupportedProjectileType
            | Choice::MissingProjectileType
            | Choice::IndeterminateProjectileType
            | Choice::MissingAmmo
            | Choice::DanglingAmmo
            | Choice::UnsupportedAmmoTarget
            | Choice::IndeterminateAmmo) => Ok(Self::Rejected(rejected)),
        }
    }
}

fn required<T>(value: &SourcedValue<T>) -> Result<Cell<&T>, ReducedError> {
    let cell = completeness::cell(PATH, value)?;
    cell.value.ok_or(ReducedError::IncompleteScope(PATH))?;
    Ok(cell)
}

fn index<'read>(
    path: &'static str,
    targets: &RecordTargets,
    original: &'read SourcedValue<u32>,
) -> Result<Cell<&'read u32>, ReducedError> {
    let cell = required(original)?;
    let index = *cell.value.ok_or(ReducedError::IncompleteScope(PATH))?;
    targets.verify_index(path, index)?;
    Ok(cell)
}
