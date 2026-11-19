// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::{InventoryOwnerBounds, ItemAttachmentComponent, ItemDefinition, ItemSource};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::ammo_definition::{self, AmmoDefinition, AmmoSelection};
use crate::catalog_v3::catalog_entity_configurations::{self, VEHICLE_ARMOR_TYPES, VehicleArmor};
use crate::catalog_v3::catalog_item_alignment::{ATTACHABLE, AlignmentOptions};
use crate::catalog_v3::catalog_item_components::{self, WeaponSource, definition_read};
use crate::catalog_v3::inventory_entity_configurations::BOUNDS_SOURCE_TYPES;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

#[derive(Clone, Copy)]
pub(super) enum Family {
    Definitions,
    ItemSources,
    Attachments,
    Bounds,
    CargoGrids,
    VehicleArmor,
    AmmoDefinitions,
    WeaponSources,
    ProjectileAmmo,
    MagazineResolutions,
}

impl Family {
    pub(super) const fn localized(self) -> bool {
        matches!(self, Self::Definitions | Self::AmmoDefinitions)
    }

    pub(super) const fn kind(self) -> Kind {
        match self {
            Self::AmmoDefinitions => Kind::AmmoDefinitions,
            Self::Definitions
            | Self::ItemSources
            | Self::Attachments
            | Self::Bounds
            | Self::CargoGrids
            | Self::VehicleArmor
            | Self::WeaponSources
            | Self::ProjectileAmmo
            | Self::MagazineResolutions => Kind::Items,
        }
    }

    pub(super) fn availability(self, forge: &DataForge<'_>, rows: usize) -> DatasetAvailability {
        if rows != 0 {
            return DatasetAvailability::Available;
        }
        let types = match self {
            Self::Definitions | Self::ItemSources => &[p4kx_assembly::ENTITY_TYPE][..],
            Self::Attachments => &[ATTACHABLE][..],
            Self::Bounds => BOUNDS_SOURCE_TYPES,
            Self::VehicleArmor => VEHICLE_ARMOR_TYPES,
            Self::CargoGrids => &[crate::catalog_v3::catalog_facility::CARGO_GRID_SOURCE_TYPE],
            Self::AmmoDefinitions => &[ammo_definition::SOURCE_TYPE],
            Self::ProjectileAmmo | Self::MagazineResolutions => {
                &[crate::item_stats::WEAPON_COMPONENT]
            }
            Self::WeaponSources => &[
                crate::item_stats::WEAPON_COMPONENT,
                crate::item_stats::AMMO_CONTAINER,
            ],
        };
        DatasetAvailability::from_source_declarations(forge, |name| types.contains(&name))
    }
}

pub(super) trait Projector {
    type Row<'source>;
    const FAMILY: Family;

    fn read<'source>(
        forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        options: &AlignmentOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError>;
}

pub(super) struct Definitions;
pub(super) struct ItemSources;
pub(super) struct Attachments;
pub(super) struct Bounds;
pub(super) struct CargoGrids;
pub(super) struct VehicleArmorRows;
pub(super) struct AmmoDefinitions;
pub(super) struct WeaponSources;
pub(super) struct ProjectileAmmo;
pub(super) struct WeaponCombatBases;
pub(super) struct MagazineResolutions;

impl Projector for MagazineResolutions {
    type Row<'source> = super::WeaponMagazineResolution;
    const FAMILY: Family = Family::MagazineResolutions;

    fn read<'source>(
        forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &AlignmentOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        catalog_item_components::read_magazine_resolution(
            forge, tree, projection, admission, cancel,
        )
    }
}

impl Projector for WeaponCombatBases {
    type Row<'source> = super::weapon_combat_read::WeaponCombatOwner<'source>;
    const FAMILY: Family = Family::ProjectileAmmo;

    fn read<'source>(
        forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &AlignmentOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        super::weapon_combat_read::read(forge, tree, projection, admission, cancel)
    }
}

impl Projector for ProjectileAmmo {
    type Row<'source> = super::projectile_ammo_read::ProjectileAmmoOwner<'source>;
    const FAMILY: Family = Family::ProjectileAmmo;

    fn read<'source>(
        forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &AlignmentOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        super::projectile_ammo_read::read(forge, tree, projection, admission, cancel)
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
        _: &AlignmentOptions<'_>,
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
        _: &AlignmentOptions<'_>,
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

impl Projector for Definitions {
    type Row<'source> = ItemDefinition<'source>;
    const FAMILY: Family = Family::Definitions;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &AlignmentOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        let read = definition_read::read(tree, projection, admission, cancel)?;
        Ok(read.map(|row| vec![row]))
    }
}

impl Projector for ItemSources {
    type Row<'source> = ItemSource<'source>;
    const FAMILY: Family = Family::ItemSources;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &AlignmentOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        let read = definition_read::with_definition(
            tree,
            projection,
            admission,
            cancel,
            |definition, originals| crate::item::source(tree, definition, originals, projection),
        )?;
        let (_, source) = read.value;
        Ok(ScopedSourceRead {
            selection_complete: source.source_set_complete(),
            value: vec![source],
            notes: Vec::new(),
        })
    }
}

impl Projector for Attachments {
    type Row<'source> = ItemAttachmentComponent<'source>;
    const FAMILY: Family = Family::Attachments;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        options: &AlignmentOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        catalog_entity_configurations::read_attachment_constraints(
            tree, projection, admission, options, cancel,
        )
    }
}

impl Projector for CargoGrids {
    type Row<'source> = super::cargo_read::CargoGridOwner<'source>;
    const FAMILY: Family = Family::CargoGrids;

    fn read<'source>(
        forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        options: &AlignmentOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        super::cargo_read::read(forge, tree, projection, admission, options, cancel)
    }
}

impl Projector for Bounds {
    type Row<'source> = InventoryOwnerBounds<'source>;
    const FAMILY: Family = Family::Bounds;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        options: &AlignmentOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        catalog_entity_configurations::read_item_bounds(
            tree, projection, admission, options, cancel,
        )
    }
}

impl Projector for VehicleArmorRows {
    type Row<'source> = VehicleArmor<'source>;
    const FAMILY: Family = Family::VehicleArmor;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        options: &AlignmentOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        catalog_entity_configurations::read_vehicle_armor(
            tree, projection, admission, options, cancel,
        )
    }
}
