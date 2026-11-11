// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForgeError, SourceCoordinateOmission, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceValue,
};

use super::OriginalDatum;
use super::health_text_capture::CapturedText;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read_failure::CatalogReadFailure;

/// Captures typed formatter contributions from the same failed canonical read.
pub(crate) fn capture(
    input: &OriginalCatalogInput<'_, '_>,
    failure: &CatalogReadFailure<'_>,
) -> Result<OriginalDatum<CapturedText>, CatalogError> {
    let record = failure.record();
    let context = input.record_context(record)?;
    let _attempt = context.observe("", SourceUseRole::Attempted)?;
    let mut contributions = Vec::new();
    if let Some(error) = failure.canonical_error() {
        canonical(error, context.dataforge(), &mut contributions)?;
    }
    input
        .origins()
        .health_texts()
        .capture(OriginalDatum::observed(
            SourceValue::Present(failure.reason()),
            contributions,
            false,
        ))
}

fn canonical(
    error: &DataForgeError,
    scope: &SourceScope,
    contributions: &mut Vec<SourceUse>,
) -> Result<(), CatalogError> {
    match error {
        DataForgeError::StructLayout { struct_index, .. } => {
            contributions.push(structure(
                scope,
                *struct_index,
                "index",
                SourceUseRole::Selector,
            )?);
            contributions.push(structure(
                scope,
                *struct_index,
                "size",
                SourceUseRole::Conversion,
            )?);
            contributions.push(unavailable(scope, SourceUseRole::Conversion)?);
        }
        DataForgeError::StructCycle { struct_index } => {
            contributions.push(structure(
                scope,
                *struct_index,
                "index",
                SourceUseRole::Selector,
            )?);
            contributions.push(unavailable(scope, SourceUseRole::Selector)?);
        }
        DataForgeError::UnknownDataType { .. }
        | DataForgeError::InvalidString { .. }
        | DataForgeError::IndexOutOfRange { .. } => {
            contributions.push(unavailable(scope, SourceUseRole::Conversion)?);
        }
        DataForgeError::TooLarge { .. } => {
            contributions.push(unavailable(scope, SourceUseRole::Selector)?);
        }
        DataForgeError::ObservationStopped => return Err(CatalogError::Cancelled),
        DataForgeError::UnsupportedVersion { .. }
        | DataForgeError::Truncated { .. }
        | DataForgeError::DuplicateMapping { .. }
        | DataForgeError::InstanceSize { .. }
        | DataForgeError::MaterializationBudgetExceeded { .. }
        | DataForgeError::MaterializationAllocationRefused { .. }
        | DataForgeError::LayoutAllocationRefused { .. }
        | DataForgeError::LayoutInvariant
        | DataForgeError::CanonicalTreeInvariant => return Ok(()),
        _ => {}
    }
    Ok(())
}

fn structure(
    scope: &SourceScope,
    index: u32,
    field: &str,
    role: SourceUseRole,
) -> Result<SourceUse, CatalogError> {
    Ok(scope.at(
        SourceLocation::Subresource {
            namespace: "dataforgeStruct".to_owned(),
            indices: vec![u64::from(index)],
            field: field.to_owned(),
        },
        role,
    )?)
}

fn unavailable(scope: &SourceScope, role: SourceUseRole) -> Result<SourceUse, CatalogError> {
    Ok(scope.at(
        SourceLocation::Opaque {
            reason: SourceCoordinateOmission::Unavailable,
        },
        role,
    )?)
}
