// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::{InventoryOwnerBounds, ItemAttachmentComponent, ItemDefinition};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::catalog_entity_configurations;
use crate::catalog_v3::catalog_item_alignment::{ATTACHABLE, AlignmentOptions};
use crate::catalog_v3::catalog_item_components::definition_read;
use crate::catalog_v3::inventory_entity_configurations::BOUNDS_SOURCE_TYPES;
use crate::record_observation::RecordObservation;

#[derive(Clone, Copy)]
pub(super) enum Family {
    Definitions,
    Attachments,
    Bounds,
}

impl Family {
    pub(super) const fn localized(self) -> bool {
        matches!(self, Self::Definitions)
    }

    pub(super) fn availability(self, forge: &DataForge<'_>, rows: usize) -> DatasetAvailability {
        if rows != 0 {
            return DatasetAvailability::Available;
        }
        let types = match self {
            Self::Definitions => &[p4kx_assembly::ENTITY_TYPE][..],
            Self::Attachments => &[ATTACHABLE][..],
            Self::Bounds => BOUNDS_SOURCE_TYPES,
        };
        DatasetAvailability::from_source_declarations(forge, |name| types.contains(&name))
    }
}

pub(super) trait Projector {
    type Row<'source>;
    const FAMILY: Family;

    fn read<'source>(
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        options: &AlignmentOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError>;
}

pub(super) struct Definitions;
pub(super) struct Attachments;
pub(super) struct Bounds;

impl Projector for Definitions {
    type Row<'source> = ItemDefinition<'source>;
    const FAMILY: Family = Family::Definitions;

    fn read<'source>(
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

impl Projector for Attachments {
    type Row<'source> = ItemAttachmentComponent<'source>;
    const FAMILY: Family = Family::Attachments;

    fn read<'source>(
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

impl Projector for Bounds {
    type Row<'source> = InventoryOwnerBounds<'source>;
    const FAMILY: Family = Family::Bounds;

    fn read<'source>(
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
