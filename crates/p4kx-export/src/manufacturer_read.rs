// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Guid, RecordTree, StructValue};

use super::manufacturer_definition::{
    Manufacturer, ManufacturerDefinition, ManufacturerLocalization,
};
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::catalog_table::CatalogTable;
use crate::guid_text::CatalogGuid;
use crate::source_fields::FieldError;

pub(crate) struct ManufacturerRead<'source> {
    pub(crate) value: Manufacturer<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Prepares exact Manufacturer fields from one canonical Record tree without following targets.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
) -> Result<ManufacturerRead<'source>, CatalogError> {
    let mut notes = Vec::new();
    if tree.guid() == Guid::default() {
        notes.push(FieldError::Malformed {
            source_type: tree.type_name().to_owned(),
            field: "$guid".to_owned(),
            expected: "a non-nil Record identity",
        });
    }
    let context = projection.origins().record(tree.record_index());
    let fields = CatalogFields::new(tree.root(), "", &context);
    fields.check(&["Code", "Localization"], &mut notes);
    let localization = fields
        .structure("Localization", &mut notes)?
        .try_map(|source| {
            localization(
                source,
                tree.record_index(),
                projection,
                &context,
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
    notes: &mut Vec<FieldError>,
) -> Result<ManufacturerLocalization<'source>, CatalogError> {
    let fields = CatalogFields::new(source, "Localization", context);
    fields.check(&["Name", "Description"], notes);
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
