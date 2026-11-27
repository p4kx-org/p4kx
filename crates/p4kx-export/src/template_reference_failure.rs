// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, Value, ValueError};

use crate::catalog_read::original::{OriginalErrorParameter, OriginalValueFact, OriginalValuePart};
use crate::{CatalogError, ExportAccountingError};

/// Reuses only original error parameters that actually establish an unresolved selection.
pub(super) fn inputs(
    fact: &OriginalValueFact<'_, '_, '_>,
) -> Result<Option<Vec<SourceUse>>, CatalogError> {
    let pointer = match fact.value() {
        Value::Error(
            ValueError::InvalidPointer {
                struct_index,
                instance_index,
            }
            | ValueError::OutsideRecord {
                struct_index,
                instance_index,
            },
        ) => Some((*struct_index, *instance_index)),
        Value::Error(ValueError::DanglingReference { .. }) => {
            let guid = fact
                .dangling_guid(SourceUseRole::Selector)?
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            let (uses, complete) = guid.contributions();
            if !complete {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            let mut result = uses.to_vec();
            result.push(fact.observe(
                OriginalValuePart::Error(OriginalErrorParameter::Guid),
                SourceUseRole::Conversion,
            )?);
            return Ok(Some(result));
        }
        _ => None,
    };
    let Some((structure, instance)) = pointer else {
        return Ok(None);
    };
    let mut result = Vec::new();
    for (parameter, value) in [
        (OriginalErrorParameter::StructIndex, structure),
        (OriginalErrorParameter::InstanceIndex, instance),
    ] {
        let (part, role) = fact.pointer_error_input(parameter, value)?;
        result.push(fact.observe(part, role)?);
        result.push(fact.observe(part, SourceUseRole::Conversion)?);
    }
    Ok(Some(result))
}
