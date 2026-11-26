// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};

use super::ReducedError;
use crate::catalog_queries::{CatalogRecordCoverage, DatasetAvailability};

pub(super) fn inventory(
    path: &'static str,
    availability: DatasetAvailability,
    coverage: &CatalogRecordCoverage,
) -> Result<(), ReducedError> {
    if matches!(availability, DatasetAvailability::Unavailable)
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
    {
        return Err(ReducedError::IncompleteScope(path));
    }
    Ok(())
}

pub(super) fn all_fields(
    path: &'static str,
    availability: DatasetAvailability,
    coverage: &CatalogRecordCoverage,
) -> Result<(), ReducedError> {
    inventory(path, availability, coverage)?;
    if !coverage.incomplete_fields.is_empty() {
        return Err(ReducedError::IncompleteScope(path));
    }
    Ok(())
}

pub(super) fn value<T>(path: &'static str, value: &SourcedValue<T>) -> Result<(), ReducedError> {
    if value.origin().source_set_complete()
        && matches!(
            value.state(),
            SourceValueState::Present | SourceValueState::Absent | SourceValueState::Null
        )
    {
        Ok(())
    } else {
        Err(ReducedError::IncompleteScope(path))
    }
}

pub(super) fn rows(
    path: &'static str,
    observed: usize,
    emitted: usize,
) -> Result<(), ReducedError> {
    if observed != emitted {
        return Err(ReducedError::IncompleteScope(path));
    }
    Ok(())
}
