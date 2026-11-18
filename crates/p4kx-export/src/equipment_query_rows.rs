// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::{InventoryOwnerBounds, ItemAttachmentComponent, ItemDefinition, ItemSource};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::ammo_definition::{self, AmmoDefinition, AmmoSelection};
use crate::catalog_v3::catalog_entity_configurations;
use crate::catalog_v3::catalog_item_alignment::{ATTACHABLE, AlignmentOptions};
use crate::catalog_v3::catalog_item_components::definition_read;
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
    AmmoDefinitions,
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
            | Self::CargoGrids => Kind::Items,
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
            Self::CargoGrids => &[crate::catalog_v3::catalog_facility::CARGO_GRID_SOURCE_TYPE],
            Self::AmmoDefinitions => &[ammo_definition::SOURCE_TYPE],
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
pub(super) struct AmmoDefinitions;

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
