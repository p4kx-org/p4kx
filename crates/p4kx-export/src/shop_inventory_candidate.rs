// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, OriginalDatum};
use crate::schema_fields::every_field_required;
use crate::shop_containers::Component;
use crate::shop_inventory_name;
use crate::shop_inventory_observations::{InventoryObservations, RejectedInventoryPath};
use crate::{CatalogError, ExportAccountingError, HealthText};

/// One original occurrence consulted for the shop inventory declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(super) enum InventoryCandidate {
    File {
        source_path: SourcedValue<String>,
        stem: SourcedValue<String>,
    },
    NoDeclaredFile,
    Unscanned,
    Rejected {
        #[schemars(extend("x-gameText" = true))]
        source_path: SourcedValue<String>,
    },
}

pub(super) struct CapturedCandidates {
    pub(super) values: Vec<SourcedValue<InventoryCandidate>>,
    pub(super) uses: Vec<SourceUse>,
    pub(super) complete: bool,
}

impl CapturedCandidates {
    pub(super) fn read(
        states: Option<(&[Component], &InventoryObservations)>,
        capture: &EntryCapture<'_>,
        declaration_complete: bool,
    ) -> Result<Self, CatalogError> {
        let mut result = Self {
            values: Vec::new(),
            uses: Vec::new(),
            complete: declaration_complete && states.is_some(),
        };
        let Some((states, observed)) = states else {
            return Ok(result);
        };
        for state in states {
            if let Component::File(stem) = state {
                for path in stem.declarations() {
                    result.file(path, capture)?;
                }
            }
        }
        for path in &observed.conflict_paths {
            result.file(path, capture)?;
        }
        result.rejected(&observed.rejected, capture)?;
        for marker in &observed.no_file {
            result.marker(marker, InventoryCandidate::NoDeclaredFile, capture)?;
        }
        for metadata in &observed.unscanned {
            result.marker(metadata, InventoryCandidate::Unscanned, capture)?;
            result.complete = false;
        }
        Ok(result)
    }

    fn file(
        &mut self,
        path: &AuthoredEntryPath,
        capture: &EntryCapture<'_>,
    ) -> Result<(), CatalogError> {
        let original = path.retain_in(capture)?;
        let normalized = path
            .clone()
            .convert(capture, shop_inventory_name::stem)?
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?
            .retain_in(capture)?;
        let sources = capture.source_uses();
        let candidate = InventoryCandidate::File {
            source_path: text_cell(&original, sources)?,
            stem: text_cell(&normalized, sources)?,
        };
        self.add(&original, candidate, capture)
    }

    pub(super) fn rejected(
        &mut self,
        paths: &[RejectedInventoryPath],
        capture: &EntryCapture<'_>,
    ) -> Result<(), CatalogError> {
        for path in paths {
            let (original, source_path) = match path {
                RejectedInventoryPath::Text(token) => {
                    let original = token.retain_in(capture)?;
                    let cell = text_cell(&original, capture.source_uses())?;
                    (original, cell)
                }
                RejectedInventoryPath::Unreadable(property) => (
                    property.retain_in(capture)?,
                    capture
                        .source_uses()
                        .value(SourceValue::Unreadable, &[], false),
                ),
            };
            self.add(
                &original,
                InventoryCandidate::Rejected { source_path },
                capture,
            )?;
            self.complete = false;
        }
        Ok(())
    }

    fn marker(
        &mut self,
        token: &AuthoredEntryPath,
        value: InventoryCandidate,
        capture: &EntryCapture<'_>,
    ) -> Result<(), CatalogError> {
        self.add(&token.retain_in(capture)?, value, capture)
    }

    fn add(
        &mut self,
        original: &OriginalDatum<HealthText>,
        candidate: InventoryCandidate,
        capture: &EntryCapture<'_>,
    ) -> Result<(), CatalogError> {
        let (uses, complete) = original.contributions();
        self.complete &= complete;
        extend(&mut self.uses, uses)?;
        self.values
            .try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        self.values.push(capture.source_uses().value(
            SourceValue::Present(candidate),
            uses,
            complete,
        ));
        Ok(())
    }
}

pub(super) fn text_cell(
    original: &OriginalDatum<HealthText>,
    sources: &SourceUses,
) -> Result<SourcedValue<String>, ExportAccountingError> {
    let SourceValue::Present(value) = original.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum);
    };
    let (uses, complete) = original.contributions();
    Ok(sources.value(
        SourceValue::Present(value.as_str().to_owned()),
        uses,
        complete,
    ))
}

pub(super) fn extend(target: &mut Vec<SourceUse>, uses: &[SourceUse]) -> Result<(), CatalogError> {
    target
        .try_reserve(uses.len())
        .map_err(CatalogError::SourceUseAllocation)?;
    target.extend_from_slice(uses);
    Ok(())
}
