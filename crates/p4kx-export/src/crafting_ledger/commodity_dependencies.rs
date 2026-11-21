// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{CancelToken, DataForge, Guid, Record, RecordTree, TreeBuilder};

use super::mode::ReadMode;
use super::selection::{self, Action, ReadPolicy, Selection};
use crate::CatalogError;
use crate::blueprint_backfill_source::{self, BlueprintResources};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_query_session;
use crate::crafting_records;
use crate::crafting_tier;
use crate::export_ledger::ExportLedger;
use crate::quality_catalog::{Assembly, Fallbacks, Step};
use crate::record_observation::RecordObservation;

pub(crate) struct CommodityDependencies {
    pub(crate) blueprints: Vec<BlueprintResources>,
    pub(crate) quality: Fallbacks,
    pub(crate) coverage: CatalogRecordCoverage,
}

impl CommodityDependencies {
    pub(crate) fn read<'source>(
        forge: &'source DataForge<'source>,
        projection: &CatalogProjection<'_, 'source>,
        wanted_resources: &HashSet<Guid>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut blueprints = Vec::new();
        let mut coverage = CatalogRecordCoverage::default();
        let mut quality = Assembly::default();
        let mut trees = TreeBuilder::new();
        let input = projection.original();
        let mode = ReadMode::catalog(projection);
        for record in forge.records() {
            catalog_query_session::checkpoint(cancel)?;
            let Some(selected) = candidate(&record, wanted_resources, &mut coverage) else {
                continue;
            };
            if selected.policy == ReadPolicy::SkipBefore {
                coverage.skipped_leftovers += 1;
                continue;
            }
            let Ok(tree) = trees.build(&record) else {
                coverage.unreadable_bodies.push(record.index());
                quality.add(mode.failed(&record, Some(selected.action))?)?;
                continue;
            };
            let admission = RecordObservation::capture(input, &record)?;
            if selected.action == Action::Blueprint {
                blueprints.push(blueprint_resource_inputs(
                    &tree, projection, &admission, cancel,
                )?);
                continue;
            }
            let mut ledger = ExportLedger::default();
            let step = mode.project(selected, &admission, &tree, &mut ledger, cancel)?;
            if !ledger.derived_failures.is_empty()
                || ledger.field_diagnostics.iter().any(|note| {
                    note.severity == crate::export_diagnostic::ExportDiagnosticSeverity::Error
                })
            {
                coverage.incomplete_fields.push(record.index());
            }
            quality.add(Step {
                input: step,
                ..Step::default()
            })?;
        }
        Ok(Self {
            blueprints,
            quality: quality.finish(projection.origins(), cancel)?.fallbacks,
            coverage,
        })
    }
}

fn candidate(
    record: &Record<'_>,
    wanted: &HashSet<Guid>,
    coverage: &mut CatalogRecordCoverage,
) -> Option<Selection> {
    match selection::choose(record, None) {
        Ok(Some(selected))
            if matches!(selected.action, Action::Blueprint | Action::Quantization)
                || selected.action == Action::ResourceLinks && wanted.contains(&record.guid()) =>
        {
            Some(selected)
        }
        Err(_) => {
            coverage.unreadable_headers.push(record.index());
            None
        }
        _ => None,
    }
}

fn blueprint_resource_inputs<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintResources, CatalogError> {
    let scope = projection
        .original()
        .record_scope(admission.record(), tree)?;
    let fields = crate::catalog_read::original::OriginalFlowFields::read(&scope, cancel)?;
    let source = crafting_records::blueprint_source(tree);
    let interpreted = crafting_tier::tiers(source, tree, &mut Vec::new(), cancel)?;
    blueprint_backfill_source::read(&interpreted, &fields)
}
