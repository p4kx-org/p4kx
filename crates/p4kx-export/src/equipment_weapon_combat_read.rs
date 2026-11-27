// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::projectile_ammo_read::{self, ProjectileAmmoOwner, ProjectileInputs};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_weapon_actions::WeaponActions;
use crate::catalog_v3::weapon_combat_base::{self, WeaponCombatBase};
use crate::record_observation::RecordObservation;

/// Canonical combat modes and reached ammunition of one physical Item.
#[derive(Debug)]
#[non_exhaustive]
pub struct WeaponCombatOwner<'source> {
    /// The original fire-action slots and their combat outcomes in Catalog order.
    pub combat_bases: Vec<WeaponCombatBase<'source>>,
    /// The same read's projectile routes, reached Ammo and dependency coverage.
    pub projectile_ammo: ProjectileAmmoOwner<'source>,
}

pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<WeaponCombatOwner<'source>>>, CatalogError> {
    Ok(
        prepare(forge, tree, projection, admission, cancel)?.map(|read| {
            (!read.bases.is_empty())
                .then(|| read.into_owner(forge))
                .into_iter()
                .collect()
        }),
    )
}

struct PreparedCombat<'source> {
    bases: Vec<WeaponCombatBase<'source>>,
    inputs: ProjectileInputs<'source>,
}

impl<'source> PreparedCombat<'source> {
    fn into_owner(self, forge: &DataForge<'_>) -> WeaponCombatOwner<'source> {
        WeaponCombatOwner {
            combat_bases: self.bases,
            projectile_ammo: self.inputs.into_owner(forge),
        }
    }
}

fn prepare<'source>(
    forge: &'source DataForge<'source>,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<PreparedCombat<'source>>, CatalogError> {
    let read = projectile_ammo_read::read_inputs(forge, tree, projection, admission, cancel)?;
    let combat_bases = weapon_combat_base::rows(
        &projection.origins().uses,
        &read.value.actions,
        &read.value.selections,
        &read.value.ammunition,
        cancel,
    )?;
    Ok(read.map(|inputs| PreparedCombat {
        bases: combat_bases,
        inputs,
    }))
}

pub(crate) struct CombinedWeaponOwner<'source> {
    pub(crate) actions: Vec<WeaponActions<'source>>,
    pub(crate) combat: WeaponCombatOwner<'source>,
}

pub(super) fn read_combined<'source>(
    forge: &'source DataForge<'source>,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<CombinedWeaponOwner<'source>>>, CatalogError> {
    Ok(
        prepare(forge, tree, projection, admission, cancel)?.map(|mut read| {
            (!(read.inputs.actions.is_empty() && read.inputs.selections.is_empty()))
                .then(|| CombinedWeaponOwner {
                    actions: std::mem::take(&mut read.inputs.actions),
                    combat: read.into_owner(forge),
                })
                .into_iter()
                .collect()
        }),
    )
}
