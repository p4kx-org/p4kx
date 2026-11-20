// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, RecordTree};

use super::{InventoryOwnerBounds, ItemAttachmentComponent, ItemDefinition, ItemSource};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::ammo_definition;
use crate::catalog_v3::catalog_entity_configurations::{
    self, FuelConfigurationKind, FuelIntake, FuelTank, RESOURCE_COMPONENT_TYPES,
    RESOURCE_CONNECTION_TYPES, ResourceComponent, ResourceConnection, ResourceContainer,
    VEHICLE_ARMOR_TYPES, VehicleArmor,
};
use crate::catalog_v3::catalog_item_alignment::{ATTACHABLE, AlignmentOptions};
use crate::catalog_v3::catalog_item_components::{ItemEquipmentFamily, definition_read};
use crate::catalog_v3::inventory_entity_configurations::BOUNDS_SOURCE_TYPES;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

#[derive(Clone, Copy)]
pub(super) enum ConfigurationFamily {
    VehicleArmor,
    ResourceComponents,
    ResourceConnections,
    Fuel(FuelConfigurationKind),
}

impl ConfigurationFamily {
    pub(super) const fn source_types(self) -> &'static [&'static str] {
        match self {
            Self::VehicleArmor => VEHICLE_ARMOR_TYPES,
            Self::ResourceComponents => RESOURCE_COMPONENT_TYPES,
            Self::ResourceConnections => RESOURCE_CONNECTION_TYPES,
            Self::Fuel(configuration) => configuration.source_types(),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Family {
    Definitions,
    ItemSources,
    Attachments,
    Bounds,
    CargoGrids,
    InventoryContainers,
    CargoLoadingGlobals,
    ShopCommodityGlobals,
    Configuration(ConfigurationFamily),
    ItemEquipment(ItemEquipmentFamily),
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
            Self::InventoryContainers => Kind::InventoryContainers,
            Self::CargoLoadingGlobals => Kind::CargoLoadingGlobals,
            Self::ShopCommodityGlobals => Kind::ShopCommodityGlobals,
            Self::Definitions
            | Self::ItemSources
            | Self::Attachments
            | Self::Bounds
            | Self::CargoGrids
            | Self::Configuration(_)
            | Self::ItemEquipment(_)
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
            Self::Configuration(family) => family.source_types(),
            Self::ItemEquipment(family) => family.source_types(),
            Self::CargoGrids => &[crate::catalog_v3::catalog_facility::CARGO_GRID_SOURCE_TYPE],
            Self::InventoryContainers | Self::CargoLoadingGlobals | Self::ShopCommodityGlobals => {
                return DatasetAvailability::from_source_declarations(forge, |name| {
                    name == self.kind().records().type_name
                });
            }
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

pub(super) struct ReadOptions<'query> {
    pub(super) alignment: AlignmentOptions<'query>,
    pub(super) item_filter: Option<&'query Filter>,
}

pub(super) trait Projector {
    type Row<'source>;
    const FAMILY: Family;

    fn read<'source>(
        forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        options: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError>;
}

pub(super) struct Definitions;
pub(super) struct ItemSources;
pub(super) struct Attachments;
pub(super) struct Bounds;
pub(super) struct CargoGrids;

impl Projector for Definitions {
    type Row<'source> = ItemDefinition<'source>;
    const FAMILY: Family = Family::Definitions;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
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
        _: &ReadOptions<'_>,
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
        options: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        catalog_entity_configurations::read_attachment_constraints(
            tree,
            projection,
            admission,
            &options.alignment,
            cancel,
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
        options: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        super::cargo_read::read(
            forge,
            tree,
            projection,
            admission,
            &options.alignment,
            cancel,
        )
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
        options: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        catalog_entity_configurations::read_item_bounds(
            tree,
            projection,
            admission,
            &options.alignment,
            cancel,
        )
    }
}

macro_rules! configuration_projector {
    ($projector:ident, $row:ident, $family:expr, $read:ident) => {
        pub(super) struct $projector;

        impl Projector for $projector {
            type Row<'source> = $row<'source>;
            const FAMILY: Family = Family::Configuration($family);

            fn read<'source>(
                _: &'source DataForge<'source>,
                tree: &RecordTree<'source>,
                projection: &CatalogProjection<'_, 'source>,
                admission: &RecordObservation<'_, 'source>,
                options: &ReadOptions<'_>,
                cancel: Option<&CancelToken>,
            ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
                catalog_entity_configurations::$read(
                    tree,
                    projection,
                    admission,
                    &options.alignment,
                    cancel,
                )
            }
        }
    };
}

configuration_projector!(
    VehicleArmorRows,
    VehicleArmor,
    ConfigurationFamily::VehicleArmor,
    read_vehicle_armor
);
configuration_projector!(
    ResourceConnectionRows,
    ResourceConnection,
    ConfigurationFamily::ResourceConnections,
    read_resource_connections
);
configuration_projector!(
    ResourceComponentRows,
    ResourceComponent,
    ConfigurationFamily::ResourceComponents,
    read_resource_components
);
configuration_projector!(
    FuelTankRows,
    FuelTank,
    ConfigurationFamily::Fuel(FuelConfigurationKind::Tank),
    read_fuel_tanks
);
configuration_projector!(
    FuelIntakeRows,
    FuelIntake,
    ConfigurationFamily::Fuel(FuelConfigurationKind::Intake),
    read_fuel_intakes
);
configuration_projector!(
    ResourceContainerRows,
    ResourceContainer,
    ConfigurationFamily::Fuel(FuelConfigurationKind::Container),
    read_resource_containers
);
