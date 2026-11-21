// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_extraction::{CancelToken, CanonicalReadObserver, Guid, Record, TreeBuilder};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_query_session::checkpoint;
use crate::entity_selection::{self, Selected};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{SourceDisposition, SourceObservation};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

#[derive(Default)]
pub(super) struct Categories {
    pub(super) missing: HashSet<Guid>,
    pub(super) sources: Vec<SourceObservation>,
}

impl Categories {
    pub(super) fn read(
        input: &OriginalCatalogInput<'_, '_>,
        wanted: &HashSet<Guid>,
        coverage: &mut CatalogRecordCoverage,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut result = Self::default();
        let mut trees = TreeBuilder::new();
        trees.set_read_observer(observer);
        if wanted.is_empty() {
            return Ok(result);
        }
        for record in input.forge().records() {
            checkpoint(cancel)?;
            if !wanted.contains(&record.guid()) {
                continue;
            }
            let Some(selected) = result.select(input, &record, coverage)? else {
                continue;
            };
            let admission = RecordObservation::capture(input, &record)?;
            let disposition = if !selected {
                coverage.skipped_leftovers += 1;
                SourceDisposition::SkippedBeforeRead
            } else if let Ok(tree) = trees.build(&record) {
                let found = crate::blueprint_category::read(&tree, input, &admission, cancel)?;
                if found
                    .notes
                    .iter()
                    .any(|note| note.description().issue.blocks_projection())
                {
                    coverage.incomplete_fields.push(record.index());
                }
                SourceDisposition::Projected
            } else {
                coverage.unreadable_bodies.push(record.index());
                SourceDisposition::ReadFailed
            };
            if disposition != SourceDisposition::Projected {
                result.missing.insert(record.guid());
            }
            result.sources.push(admission.source(
                Some(Kind::BlueprintCategories),
                ExportSourceRole::Dependency,
                disposition,
            )?);
        }
        Ok(result)
    }

    fn select(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
        coverage: &mut CatalogRecordCoverage,
    ) -> Result<Option<bool>, CatalogError> {
        let rule = Kind::BlueprintCategories.records();
        let path = record.record_path();
        let source_type = record.type_name();
        if path.as_ref().is_ok_and(|path| !rule.matches_path(path))
            || source_type
                .as_ref()
                .is_ok_and(|name| *name != rule.type_name)
        {
            return Ok(None);
        }
        Ok(match entity_selection::select(record, None) {
            Ok(Some(Selected::Entity(Kind::BlueprintCategories))) => Some(true),
            Ok(Some(Selected::Leftover(Kind::BlueprintCategories))) => Some(false),
            Ok(_) => None,
            Err(_) => {
                coverage.unreadable_headers.push(record.index());
                let kind =
                    (path.is_ok() && source_type.is_ok()).then_some(Kind::BlueprintCategories);
                if kind.is_some() {
                    self.missing.insert(record.guid());
                }
                self.sources
                    .push(RecordObservation::capture(input, record)?.source(
                        kind,
                        ExportSourceRole::Dependency,
                        SourceDisposition::ReadFailed,
                    )?);
                None
            }
        })
    }
}
