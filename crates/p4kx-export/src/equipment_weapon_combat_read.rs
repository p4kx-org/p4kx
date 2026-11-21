// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::projectile_ammo_read::{self, ProjectileAmmoOwner};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
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
    let read = projectile_ammo_read::read_inputs(forge, tree, projection, admission, cancel)?;
    let combat_bases = weapon_combat_base::rows(
        &projection.origins().uses,
        &read.value.actions,
        &read.value.selections,
        &read.value.ammunition,
        cancel,
    )?;
    Ok(read.map(|inputs| {
        (!combat_bases.is_empty())
            .then(|| WeaponCombatOwner {
                combat_bases,
                projectile_ammo: inputs.into_owner(forge),
            })
            .into_iter()
            .collect()
    }))
}
