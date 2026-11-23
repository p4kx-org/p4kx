// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, FieldOrigin, RecordTree, SourceUseRole, SourcedValue};

use super::{Observed, PreparedDefinition, SOURCE_TYPE};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

pub(super) fn location_membership(
    tree: &RecordTree<'_>,
    origins: &CatalogOrigins,
) -> Result<FieldOrigin, CatalogError> {
    let record = origins.record(tree.record_index());
    let mut uses = Vec::new();
    for field in [
        RecordHeaderField::Guid,
        RecordHeaderField::SourceType,
        RecordHeaderField::RecordName,
    ] {
        record.append_header_input(
            RecordHeaderInput::Field(field),
            SourceUseRole::Selector,
            &mut uses,
        )?;
    }
    record.append_header_input(
        RecordHeaderInput::NormalizedPath,
        SourceUseRole::Selector,
        &mut uses,
    )?;
    Ok(origins.uses.origin(&uses, true))
}

pub(super) fn location_marker(
    fields: Observed<'_, '_, '_, '_>,
) -> Result<SourcedValue<bool>, CatalogError> {
    fields.predicate("description", crate::starmap::is_mining_claim)
}

/// Reads the original ancestry marker of an admitted physical Starmap Record.
pub(crate) fn read_location_marker<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<SourcedValue<bool>, CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let (prepared, _) =
        PreparedDefinition::read_original(tree, input, admission, cancel, |_, fields, _| {
            location_marker(fields)
        })?;
    let origins = input.origins();
    let membership = location_membership(tree, origins)?;
    Ok(origins
        .uses
        .add_value_origins(prepared.into_definition(), &[&membership])?)
}
