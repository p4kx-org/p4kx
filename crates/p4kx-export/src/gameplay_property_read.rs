// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree, SourceValue, SourcedValue};

use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::NumberCapture;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::CatalogFields;
use crate::catalog_table::CatalogTable;
use crate::catalog_text_contract::PublicNameRef;
use crate::guid_text::CatalogGuid;
use crate::source_fields::FieldError;

use super::contract::GameplayPropertyDefinition;
use super::{overrides, transformation};

pub(crate) struct GameplayPropertyRead<'source> {
    pub(crate) value: GameplayPropertyDefinition<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Reads original authored fields before interpreted crafting reduction.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<GameplayPropertyRead<'source>, CatalogError> {
    let numbers = NumberCapture::default();
    let context = projection
        .origins()
        .record(tree.record_index())
        .with_numbers(&numbers)
        .with_cancel(cancel);
    let fields = CatalogFields::new(tree.root(), "", &context);
    let mut notes = Vec::new();
    fields.check(
        &[
            "propertyName",
            "displayTransformation",
            "unitFormat",
            "nameOverrides",
        ],
        &mut notes,
    );
    let property_name = name(
        &fields,
        "propertyName",
        tree.record_index(),
        projection,
        &mut notes,
    )?;
    let unit_format = name(
        &fields,
        "unitFormat",
        tree.record_index(),
        projection,
        &mut notes,
    )?;
    let display_transformation = fields
        .structure("displayTransformation", &mut notes)?
        .try_map(|source| {
            transformation::read(source, "displayTransformation", &context, &mut notes)
        })?;
    let name_overrides = overrides::read(&fields, tree.record_index(), projection, &mut notes)?;
    Ok(GameplayPropertyRead {
        value: GameplayPropertyDefinition {
            physical_guid: CatalogGuid(tree.guid()),
            physical_path: tree.record_path(),
            numbers: numbers.finish(),
            source: CanonicalRecordHeader::read(tree, projection.origins())?,
            property_name,
            display_transformation,
            unit_format,
            name_overrides,
        },
        notes,
    })
}

pub(crate) fn name<'source>(
    fields: &CatalogFields<'_, 'source, '_>,
    field: &str,
    record_index: u32,
    projection: &CatalogProjection<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<PublicNameRef>, CatalogError> {
    projection.name(
        fields.locale(field, notes)?,
        record_index,
        &fields.location(field),
    )
}

/// Binds a concrete-class disposition to the actual original type selector.
pub(crate) fn payload<T>(
    fields: &CatalogFields<'_, '_, '_>,
    path: &str,
    value: Option<T>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<T>, CatalogError> {
    let value = value.map_or_else(
        || {
            notes.push(FieldError::Unsupported {
                source_type: fields.source().type_name().to_owned(),
                field: path.to_owned(),
            });
            SourceValue::Unsupported
        },
        SourceValue::Present,
    );
    fields.context().selector(&fields.location("$type"), value)
}

/// Emits the one canonical source table independently of selected row count.
pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<GameplayPropertyDefinition<'source>>,
) -> CatalogTable<GameplayPropertyDefinition<'source>> {
    CatalogTable::new(forge, &["CraftingGameplayPropertyDef"], rows)
}
