// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{CancelToken, DataForge, Guid, Record, TreeBuilder};

use super::ContractKey;
use super::plan::{Plan, selected};
use super::survey::{Survey, Wanted};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_query_session::checkpoint;

struct Candidate {
    index: u32,
    grants: BTreeSet<Guid>,
    property_sets: BTreeSet<Guid>,
}

pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    key: ContractKey,
    cancel: Option<&CancelToken>,
) -> Result<Plan<'source>, CatalogError> {
    let mut plan = Plan::new(input, false);
    plan.target = Some(key);
    let mut trees = TreeBuilder::new();
    let mut wanted = Wanted::default();
    let mut candidates = Vec::new();
    for record in forge.records() {
        checkpoint(cancel)?;
        if !admit(&record, input, &mut plan)? {
            continue;
        }
        let Some(tree) = build(&record, input, &mut plan, &mut trees)? else {
            continue;
        };
        let mut survey = Survey::new(input);
        let result = survey.read_tree(&tree, input, Some(key.identity()), cancel);
        let matches = survey.keys.contains(&key.identity());
        if matches {
            plan.roots.insert(record.index());
            plan.has_rows = true;
            wanted.extend_key(&survey, key.identity());
            plan.generators.insert(record.index(), tree);
        }
        if let Err(error) = result {
            plan.failed_survey(&record, input, error)?;
        }
        if !matches {
            candidates.push(Candidate {
                index: record.index(),
                grants: survey.grants.tag_guids().collect(),
                property_sets: std::mem::take(&mut survey.property_sets),
            });
        }
        plan.grants.merge(survey.grants.scope_only())?;
    }
    if plan.has_rows {
        select_dependencies(
            forge, input, &mut plan, &wanted, candidates, &mut trees, cancel,
        )?;
    }
    Ok(plan)
}

fn admit<'source>(
    record: &Record<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    plan: &mut Plan<'source>,
) -> Result<bool, CatalogError> {
    let mut coverage = CatalogRecordCoverage::default();
    if selected(record, &mut coverage) {
        return Ok(true);
    }
    if !coverage.unreadable_headers.is_empty() {
        plan.uncertain(record, input)?;
        plan.coverage
            .dependencies
            .unreadable_headers
            .push(record.index());
    }
    Ok(false)
}

fn build<'source>(
    record: &Record<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    plan: &mut Plan<'source>,
    trees: &mut TreeBuilder<'source>,
) -> Result<Option<p4kx_extraction::RecordTree<'source>>, CatalogError> {
    plan.coverage
        .dependencies
        .surveyed_generators
        .push(record.index());
    match trees.build(record) {
        Ok(tree) => Ok(Some(tree)),
        Err(error) => {
            plan.failed_survey(record, input, Box::new(error))?;
            Ok(None)
        }
    }
}

fn select_dependencies<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    plan: &mut Plan<'source>,
    wanted: &Wanted,
    candidates: Vec<Candidate>,
    trees: &mut TreeBuilder<'source>,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    for candidate in candidates {
        checkpoint(cancel)?;
        if candidate.grants.is_disjoint(&wanted.required_tags)
            && candidate.property_sets.is_disjoint(&wanted.property_sets)
        {
            continue;
        }
        let record = forge
            .record(candidate.index)
            .ok_or(crate::ExportAccountingError::ConflictingSourceDatum)?;
        match trees.build(&record) {
            Ok(tree) => {
                plan.coverage
                    .dependencies
                    .related_generators
                    .push(candidate.index);
                plan.generators.insert(candidate.index, tree);
            }
            Err(error) => plan.failed_survey(&record, input, Box::new(error))?,
        }
    }
    Ok(())
}
