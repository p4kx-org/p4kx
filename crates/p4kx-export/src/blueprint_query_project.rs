// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_extraction::{CancelToken, CanonicalReadObserver, Guid, SourceValue, SourcedValue};

use super::categories::Categories;
use super::dependencies::Dependencies;
use super::plan::Plan;
use super::snapshot::{BlueprintDependencyCoverage, BlueprintUnsupportedRanges};
use crate::CatalogError;
use crate::blueprint_catalog_read::{self, PreparedBlueprint};
use crate::blueprint_catalog_rows::{self, BlueprintTables};
use crate::blueprint_definition::{BlueprintDefinition, PhysicalBlueprintRef};
use crate::blueprint_presentation::BlueprintPresentation;
use crate::blueprint_rule_inputs::BlueprintRules;
use crate::blueprint_tier_contract::BlueprintTier;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_query_session::checkpoint;
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;

pub(super) struct Projected<'source> {
    pub(super) definitions: Vec<BlueprintDefinition<'source>>,
    pub(super) tiers: Vec<BlueprintTier>,
    pub(super) presentations: Vec<BlueprintPresentation<SourcedValue<CatalogGuid>>>,
    pub(super) unsupported_ranges: Vec<BlueprintUnsupportedRanges<'source>>,
    pub(super) coverage: CatalogRecordCoverage,
    pub(super) dependency_coverage: BlueprintDependencyCoverage,
}

struct Prepared<'source> {
    owners: Vec<PreparedBlueprint<'source>>,
    unsupported_ranges: Vec<BlueprintUnsupportedRanges<'source>>,
    coverage: CatalogRecordCoverage,
}

pub(super) fn project<'source>(
    plan: Plan<'source>,
    projection: &CatalogProjection<'_, 'source>,
    observer: Option<Arc<dyn CanonicalReadObserver>>,
    cancel: Option<&CancelToken>,
) -> Result<Projected<'source>, CatalogError> {
    let prepared = read_owners(plan, projection, cancel)?;
    let mut dependencies = Dependencies::new(observer.clone());
    let globals = if prepared.owners.is_empty() {
        BlueprintRules::default()
    } else {
        dependencies.globals(projection.original(), cancel)?
    };
    let wanted = category_targets(&prepared.owners);
    let categories = Categories::read(
        projection.original(),
        &wanted,
        &mut dependencies.coverage.categories,
        observer,
        cancel,
    )?;
    let mut definitions = Vec::with_capacity(prepared.owners.len());
    let mut rows = Vec::with_capacity(prepared.owners.len());
    for owner in prepared.owners {
        checkpoint(cancel)?;
        let row = blueprint_catalog_rows::row_with_process(
            owner,
            &globals,
            projection.origins(),
            || dependencies.process(projection.original(), cancel),
        )?;
        let (definition, row) = row.take_definition();
        definitions.push(definition);
        rows.push(row);
    }
    let BlueprintTables {
        tiers,
        presentation,
    } = blueprint_catalog_rows::tables(
        rows,
        &categories.missing,
        &categories.sources,
        projection.original().forge(),
        projection.origins(),
    )?;
    Ok(Projected {
        definitions,
        tiers: tiers.rows,
        presentations: presentation.rows,
        unsupported_ranges: prepared.unsupported_ranges,
        coverage: prepared.coverage,
        dependency_coverage: dependencies.coverage,
    })
}

fn read_owners<'source>(
    plan: Plan<'source>,
    projection: &CatalogProjection<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<Prepared<'source>, CatalogError> {
    let Plan {
        owners,
        mut coverage,
    } = plan;
    let mut result = Vec::with_capacity(owners.len());
    let mut unsupported_ranges = Vec::new();
    for owner in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(projection.original(), &owner.record)?;
        let read = blueprint_catalog_read::read(&owner.tree, projection, &admission, cancel)?;
        if read
            .notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        let mut value = read.value;
        if !value.unknown_range_types.is_empty() {
            unsupported_ranges.push(BlueprintUnsupportedRanges {
                blueprint: PhysicalBlueprintRef {
                    input_ref: value.definition.input_ref.clone(),
                    record_index: owner.record.index(),
                },
                source_types: std::mem::take(&mut value.unknown_range_types),
            });
        }
        result.push(value);
    }
    Ok(Prepared {
        owners: result,
        unsupported_ranges,
        coverage,
    })
}

fn category_targets(owners: &[PreparedBlueprint<'_>]) -> HashSet<Guid> {
    owners
        .iter()
        .filter_map(|owner| match owner.category.state {
            SourceValue::Present(guid) => Some(guid.get()),
            _ => None,
        })
        .collect()
}
