// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, StructValue};

use super::manufacturer_definition::{
    Manufacturer, ManufacturerDefinition, ManufacturerLocalization,
};
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::{ExampleBodyReceipt, OriginalFlowFields};
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::catalog_table::CatalogTable;
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::source_fields::{FieldCapture, FieldError};
use crate::template_field_capture::TemplateFieldCapture;

pub(crate) struct ManufacturerRead<'source> {
    pub(crate) value: Manufacturer<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Prepares exact Manufacturer fields from one canonical Record tree without following targets.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ManufacturerRead<'source>, CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let original = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&original, input, admission)?;
    let mut notes = Vec::new();
    if tree.guid() == Guid::default() {
        notes.push(
            FieldError::Malformed {
                source_type: tree.type_name().to_owned(),
                field: "$guid".to_owned(),
                expected: "a non-nil Record identity",
            }
            .with_body(ExampleBodyReceipt::technical(
                input,
                admission.subject().clone(),
                "$guid",
            )?),
        );
    }
    let context = projection
        .origins()
        .record(tree.record_index())
        .with_cancel(cancel);
    let fields = CatalogFields::observed(tree.root(), "", &context, &capture)?;
    fields.check_observed(&["Code", "Localization"], &mut notes)?;
    let localization = fields
        .structure("Localization", &mut notes)?
        .try_map(|source| {
            localization(
                source,
                tree.record_index(),
                projection,
                &context,
                &capture,
                &mut notes,
            )
        })?;
    let definition = ManufacturerDefinition {
        source: CanonicalRecordHeader::read(tree, projection.origins())?,
        code: fields.text("Code", &mut notes)?,
        localization,
    };
    Ok(ManufacturerRead {
        value: Manufacturer {
            guid: CatalogGuid(tree.guid()),
            record_path: tree.record_path(),
            definition,
        },
        notes,
    })
}

fn localization<'source>(
    source: &StructValue<'source>,
    record_index: u32,
    projection: &CatalogProjection<'_, 'source>,
    context: &CatalogRecordContext<'_>,
    capture: &dyn FieldCapture<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<ManufacturerLocalization<'source>, CatalogError> {
    let fields = CatalogFields::observed(source, "Localization", context, capture)?;
    fields.check_observed(&["Name", "Description"], notes)?;
    Ok(ManufacturerLocalization {
        source_type: fields.source_type()?,
        name: projection.name(
            fields.locale("Name", notes)?,
            record_index,
            "Localization.Name",
        )?,
        description: projection.description(
            fields.locale("Description", notes)?,
            record_index,
            "Localization.Description",
        )?,
    })
}

/// Materializes the selected definitions with their independently measured family availability.
pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<Manufacturer<'source>>,
) -> CatalogTable<ManufacturerDefinition<'source>> {
    CatalogTable::new(
        forge,
        &["SCItemManufacturer"],
        rows.into_iter().map(|row| row.definition).collect(),
    )
}
