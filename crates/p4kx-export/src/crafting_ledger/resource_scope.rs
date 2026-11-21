// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Filter, Record};

use crate::CatalogError;
use crate::crafting_quantization::{QUANTIZATIONS, RESOURCE_TYPES};
use crate::selection::is_leftover;

/// Resource inputs needed by the quantization output and selected-source diagnostics.
#[derive(Clone, Copy)]
pub(super) enum ResourceLinkScope {
    AllResources,
    SelectedResources,
}

impl ResourceLinkScope {
    pub(super) fn catalog(
        records: &[Record<'_>],
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        for record in records {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            let Ok(path) = record.record_path() else {
                return Ok(Self::AllResources);
            };
            if !QUANTIZATIONS.matches_path(path)
                || filter.is_some_and(|filter| !filter.matches(path))
            {
                continue;
            }
            match record.type_name() {
                Ok(name) if name != QUANTIZATIONS.type_name => continue,
                Err(_) => return Ok(Self::AllResources),
                Ok(_) => {}
            }
            match record.name() {
                Ok(name) if is_leftover(path, name) => {}
                Ok(_) | Err(_) => return Ok(Self::AllResources),
            }
        }
        Ok(Self::SelectedResources)
    }

    pub(super) fn includes(self, record: &Record<'_>, filter: Option<&Filter>) -> bool {
        match self {
            Self::AllResources => true,
            Self::SelectedResources => record.record_path().map_or(true, |path| {
                !RESOURCE_TYPES.matches_path(path)
                    || filter.is_none_or(|filter| filter.matches(path))
            }),
        }
    }
}

#[cfg(test)]
#[path = "resource_scope_tests.rs"]
mod tests;
