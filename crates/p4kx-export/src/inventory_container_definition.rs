// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::inventory_container_type::{self, InventoryContainerType};
use super::inventory_dimensions::InventoryVector3;
use super::inventory_item_type_filter::{self, InventoryItemTypeFilter};
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError};

pub(crate) const SOURCE_TYPE: &str = "InventoryContainer";

/// One physical authored inventory storage definition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct InventoryContainerDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    excluded_item_sub_types: SourcedValue<Vec<SourcedValue<InventoryItemTypeFilter<'source>>>>,
    interior_dimensions: SourcedValue<InventoryVector3<'source>>,
    inventory_type: SourcedValue<InventoryContainerType<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

pub(crate) struct PreparedInventoryContainer<'source> {
    definition: InventoryContainerDefinition<'source>,
    pub(crate) physical_guid: Guid,
    pub(crate) physical_index: u32,
    pub(crate) physical_path: &'source str,
    pub(crate) numbers: Vec<CapturedNumber>,
}

impl<'source> InventoryContainerDefinition<'source> {
    fn read(
        header: CanonicalRecordHeader<'source>,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let excluded_item_sub_types = source.class_array_to(
            "excludedItemSubTypes",
            inventory_item_type_filter::SOURCE_TYPE,
            notes,
            InventoryItemTypeFilter::read,
        )?;
        let interior_dimensions = source.declared_class_to(
            "interiorDimensions",
            "Vec3",
            notes,
            InventoryVector3::read,
        )?;
        let inventory_type = source.strong_pointer_family_to(
            "inventoryType",
            inventory_container_type::BASE,
            notes,
            InventoryContainerType::read,
        )?;
        let additional_fields = source.authored_members_except([
            ("excludedItemSubTypes", excluded_item_sub_types.state()),
            ("interiorDimensions", interior_dimensions.state()),
            ("inventoryType", inventory_type.state()),
        ])?;
        Ok(Self {
            source: header,
            excluded_item_sub_types,
            interior_dimensions,
            inventory_type,
            additional_fields,
        })
    }
}

/// Reads one selected inventory Record with its original field occurrences.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(PreparedInventoryContainer<'source>, Vec<FieldError>), CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let fields = Observed::definition(&originals, input, &capture, &numbers);
    let mut notes = Vec::new();
    let header = CanonicalRecordHeader::read(tree, projection.origins())?;
    let definition = InventoryContainerDefinition::read(header, fields, &mut notes)?;
    Ok((
        PreparedInventoryContainer {
            definition,
            physical_guid: tree.guid(),
            physical_index: tree.record_index(),
            physical_path: tree.record_path(),
            numbers: numbers.into_inner(),
        },
        notes,
    ))
}

/// Builds the definition table in canonical GUID and physical Record order.
pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedInventoryContainer<'source>>,
) -> CatalogTable<InventoryContainerDefinition<'source>> {
    CatalogTable::new(
        forge,
        &[SOURCE_TYPE],
        rows.into_iter().map(|row| row.definition).collect(),
    )
}
