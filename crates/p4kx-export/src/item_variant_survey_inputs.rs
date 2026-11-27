// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeError, FieldOrigin, Record, SourceLocation, SourceUseRole};

use crate::build::original_input::{self, OriginalCatalogInput};
use crate::entity_selection::{self, Selected};
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::variant_evidence::VariantEvidence;
use crate::{CatalogError, ExportAccountingError};

pub(super) struct Admission {
    pub(super) selection: Result<Option<Selected>, DataForgeError>,
    pub(super) uses: Vec<(RecordHeaderInput, SourceUseRole)>,
}

pub(super) fn admission(record: &Record<'_>) -> Admission {
    let mut uses = Vec::new();
    let selection = entity_selection::select_item_observed(record, |field, role| {
        uses.push((field, role));
    });
    Admission { selection, uses }
}

pub(super) fn observe_header(
    input: &OriginalCatalogInput<'_, '_>,
    record: &Record<'_>,
    admission: Vec<(RecordHeaderInput, SourceUseRole)>,
    complete: bool,
) -> Result<VariantEvidence, CatalogError> {
    let context = input.record_context(record)?;
    let mut observed = Vec::new();
    for (field, role) in admission {
        context.append_header_input(field, role, &mut observed)?;
    }
    for field in [RecordHeaderField::Guid, RecordHeaderField::RecordIndex] {
        observed.push(context.observe_header(field, SourceUseRole::Selector)?);
    }
    Ok(VariantEvidence::from_uses(
        &input.origins().uses,
        &observed,
        complete,
    ))
}

pub(super) fn component_census(
    input: &OriginalCatalogInput<'_, '_>,
    inspected_headers: usize,
) -> Result<FieldOrigin, CatalogError> {
    if inspected_headers != input.forge().record_count() {
        return Err(ExportAccountingError::MissingFieldInspection.into());
    }
    let uses = &input.origins().uses;
    let end =
        u64::try_from(inspected_headers).map_err(|_| ExportAccountingError::CounterOverflow)?;
    let components = input.origins().dataforge.at(
        SourceLocation::Subresource {
            namespace: "dataforgeItemComponentSurvey".to_owned(),
            indices: vec![0, end],
            field: "Components".to_owned(),
        },
        SourceUseRole::Selector,
    )?;
    let components = uses.origin(&[components], true);
    let headers = original_input::record_inventory(input)?;
    Ok(uses.origin_with_origins([&headers, &components], true)?)
}
