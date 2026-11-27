// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::projectile_ammo_read::{self, ProjectileAmmoOwner};
use super::rows::{Family, Projector, ReadOptions};
use super::weapon_combat_read::{self, WeaponCombatOwner};
use super::weapon_magazine_dependency_read;
use super::{WeaponMagazineDependencyOwner, WeaponMagazineResolution};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::ammo_definition::{self, AmmoDefinition, AmmoSelection};
use crate::catalog_v3::catalog_item_components::{self, WeaponSource};
use crate::record_observation::RecordObservation;

pub(super) struct AmmoDefinitions;
pub(super) struct WeaponSources;
pub(super) struct ProjectileAmmo;
pub(super) struct WeaponCombatBases;
pub(super) struct MagazineResolutions;
pub(super) struct MagazineDependencies;

impl Projector for MagazineResolutions {
    type Row<'source> = WeaponMagazineResolution;
    const FAMILY: Family = Family::MagazineResolutions;

    fn read<'source>(
        forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        let read = catalog_item_components::read_magazine_resolution(
            forge, tree, projection, admission, cancel, None,
        )?;
        Ok(read.map(|row| row.into_iter().collect()))
    }
}

impl Projector for MagazineDependencies {
    type Row<'source> = WeaponMagazineDependencyOwner<'source>;
    const FAMILY: Family = Family::MagazineResolutions;

    fn read<'source>(
        forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        options: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        weapon_magazine_dependency_read::read(
            forge,
            tree,
            projection,
            admission,
            cancel,
            options.item_filter,
        )
    }
}

impl Projector for WeaponCombatBases {
    type Row<'source> = WeaponCombatOwner<'source>;
    const FAMILY: Family = Family::ProjectileAmmo;

    fn read<'source>(
        forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        weapon_combat_read::read(forge, tree, projection, admission, cancel)
    }
}

impl Projector for ProjectileAmmo {
    type Row<'source> = ProjectileAmmoOwner<'source>;
    const FAMILY: Family = Family::ProjectileAmmo;

    fn read<'source>(
        forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        projectile_ammo_read::read(forge, tree, projection, admission, cancel)
    }
}

impl Projector for WeaponSources {
    type Row<'source> = WeaponSource<'source>;
    const FAMILY: Family = Family::WeaponSources;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        catalog_item_components::read_weapon_source(tree, projection, admission, cancel)
    }
}

impl Projector for AmmoDefinitions {
    type Row<'source> = AmmoDefinition<'source>;
    const FAMILY: Family = Family::AmmoDefinitions;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        let (prepared, notes) =
            ammo_definition::read(projection, tree, admission, cancel, AmmoSelection::Selected)?;
        Ok(ScopedSourceRead {
            value: vec![prepared.into_definition()],
            notes,
            selection_complete: true,
        })
    }
}
