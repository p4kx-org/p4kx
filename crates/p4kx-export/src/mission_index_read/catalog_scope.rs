// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter};

use crate::{CatalogError, Kind};

use super::selection;

/// Establishes whether selected mission inputs require the complete dependency index.
pub(in crate::mission_index) fn needed(
    forge: &DataForge<'_>,
    filter: &Filter,
    cancel: Option<&CancelToken>,
) -> Result<bool, CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    for record in forge.records() {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let Ok(path) = record.record_path() else {
            return Ok(true);
        };
        if !filter.matches(path) {
            continue;
        }
        let Ok(type_name) = record.type_name() else {
            return Ok(true);
        };
        if [
            Kind::Contracts,
            Kind::ContractTemplates,
            Kind::MissionBrokerEntries,
            Kind::MissionLocationTemplates,
        ]
        .into_iter()
        .any(|kind| kind.records().selects(path, type_name))
        {
            return Ok(true);
        }
        match selection::choose(&record) {
            Ok(Some(selection::Action::Item) | None) => {}
            Ok(Some(_)) | Err(_) => return Ok(true),
        }
    }
    Ok(false)
}

#[cfg(test)]
#[path = "catalog_scope_tests.rs"]
mod tests;
