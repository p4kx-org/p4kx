// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Record, TreeBuilder};

use crate::CatalogError;
use crate::Kind;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::checkpoint;
use crate::export_dependency_read::{self, DependencyRead};
use crate::record_observation::RecordObservation;

/// Reads one reward-survey source and supplies its original admission and field observations.
pub(crate) fn with_record<'source, T>(
    trees: &mut TreeBuilder<'source>,
    record: &Record<'source>,
    input: Option<&OriginalCatalogInput<'_, 'source>>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(
        &RecordObservation<'_, 'source>,
        DependencyRead<'source>,
    ) -> Result<T, CatalogError>,
) -> Result<T, CatalogError> {
    checkpoint(cancel)?;
    let admission = match input {
        Some(input) => RecordObservation::capture(input, record)?,
        None => RecordObservation::accounting(record),
    };
    let read =
        export_dependency_read::read(input, trees, &admission, Kind::BlueprintPools, cancel)?;
    checkpoint(cancel)?;
    let result = inspect(&admission, read)?;
    checkpoint(cancel)?;
    Ok(result)
}
