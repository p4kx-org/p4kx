// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, DataForgeError, DataForgeHeaderField, FieldOrigin, Record, SourceLocation,
    SourceUse, SourceUseRole, TreeBuilder,
};

use super::PaintSelection;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::catalog_item_components::definition_read;
use crate::entity_selection::{self, Selected};
use crate::item::{PaintInput, paint_input};
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError, Kind};

pub(super) struct Survey<'source> {
    pub(super) candidates: Vec<Owner<'source>>,
    pub(super) origin: FieldOrigin,
    pub(super) records: CatalogRecordCoverage,
}

pub(super) struct Owner<'source> {
    pub(super) index: u32,
    pub(super) inputs: PaintInput<'source>,
}

pub(super) fn read<'source>(
    projection: &CatalogProjection<'source, 'source>,
    selection: PaintSelection,
    cancel: Option<&CancelToken>,
) -> Result<Survey<'source>, CatalogError> {
    let input = projection.original();
    let mut records = CatalogRecordCoverage::default();
    let mut candidates = Vec::new();
    let mut origins = match selection {
        PaintSelection::RecordIndex(index) if input.forge().record(index).is_some() => Vec::new(),
        PaintSelection::RecordIndex(_) => {
            vec![input.origins().uses.origin(&[record_count(input)?], true)]
        }
        PaintSelection::Guid(_) => vec![record_inventory(input)?],
    };
    let mut trees = TreeBuilder::new();
    let selection = match selection {
        PaintSelection::RecordIndex(index) => CatalogRecordSelection::RecordIndex(index),
        PaintSelection::Guid(guid) => CatalogRecordSelection::Guid(guid),
    };
    selection.for_each(input.forge(), cancel, |record| {
        checkpoint(cancel)?;
        let (selected, header) = admitted(input, record, Kind::Items, &mut records)?;
        origins.push(header);
        if !selected {
            return Ok(());
        }
        let tree = match trees.build(record) {
            Ok(tree) => tree,
            Err(DataForgeError::ObservationStopped) => return Err(CatalogError::Cancelled),
            Err(_) => {
                records.unreadable_bodies.push(record.index());
                return Ok(());
            }
        };
        let admission = RecordObservation::capture(input, record)?;
        let read = definition_read::with_definition(
            &tree,
            projection,
            &admission,
            cancel,
            |definition, originals| paint_input::read(definition, originals, &tree),
        )?;
        let (_, inputs) = read.value;
        if !inputs.origin.source_set_complete() {
            records.incomplete_fields.push(record.index());
        }
        origins.push(inputs.origin.clone());
        if inputs.is_paint {
            candidates.push(Owner {
                index: record.index(),
                inputs,
            });
        }
        Ok(())
    })?;
    let origin = input
        .origins()
        .uses
        .origin_with_origins(origins.iter(), super::complete(&records))?;
    Ok(Survey {
        candidates,
        origin,
        records,
    })
}

pub(super) fn admitted(
    input: &OriginalCatalogInput<'_, '_>,
    record: &Record<'_>,
    kind: Kind,
    records: &mut CatalogRecordCoverage,
) -> Result<(bool, FieldOrigin), CatalogError> {
    let origin = input.record_context(record)?;
    let mut selected_fields = Vec::new();
    let selected = entity_selection::select_observed(record, None, |field, role| {
        selected_fields.push((field, role));
    });
    let mut uses = Vec::new();
    for (field, role) in selected_fields {
        origin.append_header_input(field, role, &mut uses)?;
    }
    origin.append_header_input(
        RecordHeaderInput::Field(RecordHeaderField::Guid),
        SourceUseRole::Selector,
        &mut uses,
    )?;
    let header = input.origins().uses.origin(&uses, selected.is_ok());
    let admitted = match selected {
        Ok(Some(Selected::Entity(actual))) if actual == kind => true,
        Ok(Some(Selected::Leftover(actual))) if actual == kind => {
            records.skipped_leftovers += 1;
            false
        }
        Err(_) => {
            records.unreadable_headers.push(record.index());
            false
        }
        _ => false,
    };
    Ok((admitted, header))
}

pub(super) fn record_inventory(
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
