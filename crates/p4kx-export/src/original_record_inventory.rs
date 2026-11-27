// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForgeHeaderField, FieldOrigin, SourceLocation, SourceUse, SourceUseRole,
};

use super::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError};

pub(crate) fn record_inventory(
    input: &OriginalCatalogInput<'_, '_>,
) -> Result<FieldOrigin, CatalogError> {
    let forge = input.forge();
    let origins = input.origins();
    let mut uses = vec![record_count(input)?];
    if forge.record_count() != 0 {
        let table = forge
            .record_table_span()
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        uses.push(span(input, "dataforgeRecordTable", "fileName", table)?);
    }
    if let Some(text) = forge.text_table_span() {
        uses.push(span(input, "dataforgeTextTable", "fileName", text)?);
    }
    Ok(origins.uses.origin(&uses, true))
}

pub(crate) fn append_record_guid_inventory(
    input: &OriginalCatalogInput<'_, '_>,
    uses: &mut Vec<SourceUse>,
) -> Result<(), CatalogError> {
    append_record_lookup_columns(input, uses, &["guid"])
}

pub(crate) fn append_record_instance_inventory(
    input: &OriginalCatalogInput<'_, '_>,
    uses: &mut Vec<SourceUse>,
) -> Result<(), CatalogError> {
    append_record_lookup_columns(input, uses, &["structIndex", "instanceIndex"])
}

fn append_record_lookup_columns(
    input: &OriginalCatalogInput<'_, '_>,
    uses: &mut Vec<SourceUse>,
    columns: &[&str],
) -> Result<(), CatalogError> {
    uses.push(record_count(input)?);
    let forge = input.forge();
    if forge.record_count() != 0 {
        let table = forge
            .record_table_span()
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        for field in columns {
            uses.push(span(input, "dataforgeRecordTable", field, table.clone())?);
        }
    }
    Ok(())
}

fn record_count(input: &OriginalCatalogInput<'_, '_>) -> Result<SourceUse, CatalogError> {
    span(
        input,
        "dataforgeHeader",
        "recordCount",
        DataForgeHeaderField::RecordCount.byte_span(),
    )
}

fn span(
    input: &OriginalCatalogInput<'_, '_>,
    namespace: &str,
    field: &str,
    range: std::ops::Range<u64>,
) -> Result<SourceUse, CatalogError> {
    Ok(input.origins().dataforge.at(
        SourceLocation::Subresource {
            namespace: namespace.to_owned(),
            indices: vec![range.start, range.end],
            field: field.to_owned(),
        },
        SourceUseRole::Selector,
    )?)
}

pub(crate) fn record_body_location(index: u32) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: "dataforgeRecordBody".to_owned(),
        indices: vec![u64::from(index)],
        field: "body".to_owned(),
    }
}
