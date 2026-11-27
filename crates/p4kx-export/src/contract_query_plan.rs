// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{CancelToken, DataForge, Record, RecordTree, SourceUseRole, TreeBuilder};

use super::survey::{Survey, Wanted};
use super::{ContractKey, ContractQueryCoverage};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_query_session::checkpoint;
use crate::contract_generator_definition;
use crate::mission_intros::Grants;
use crate::record_read::ReadError;
use crate::{CatalogError, CatalogReadFailure};

#[derive(Clone, Copy)]
pub(super) enum Lookup {
    Owners(CatalogRecordSelection),
    Logical(ContractKey),
}

pub(super) struct Plan<'source> {
    pub(super) roots: BTreeSet<u32>,
    pub(super) generators: BTreeMap<u32, RecordTree<'source>>,
    pub(super) grants: Grants,
    pub(super) coverage: ContractQueryCoverage<'source>,
    pub(super) has_rows: bool,
    pub(super) global: bool,
    pub(super) target: Option<ContractKey>,
}

impl<'source> Plan<'source> {
    pub(super) fn read(
        forge: &'source DataForge<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        selection: Lookup,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        match selection {
            Lookup::Owners(selection) => Self::read_owners(forge, input, selection, cancel),
            Lookup::Logical(key) => super::key_plan::read(forge, input, key, cancel),
        }
    }

    pub(super) fn new(input: &OriginalCatalogInput<'_, 'source>, global: bool) -> Self {
        Self {
            roots: BTreeSet::new(),
            generators: BTreeMap::new(),
            grants: Grants::new(input.origins()),
            coverage: ContractQueryCoverage::default(),
            has_rows: false,
            global,
            target: None,
        }
    }

    fn read_owners(
        forge: &'source DataForge<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut plan = Self::new(input, matches!(selection, CatalogRecordSelection::All));
        let mut trees = TreeBuilder::new();
        let mut wanted = Wanted::default();
        selection.for_each(forge, cancel, |record| {
            let unreadable = plan.coverage.selected.unreadable_headers.len();
            if selected(record, &mut plan.coverage.selected) {
                plan.roots.insert(record.index());
                if let Some((tree, survey)) = plan.survey(record, input, &mut trees, cancel)? {
                    plan.has_rows |= !survey.keys.is_empty();
                    if !plan.global {
                        wanted.extend(&survey);
                        plan.grants.merge(survey.grants.scope_only())?;
                    }
                    plan.generators.insert(record.index(), tree);
                }
            } else if unreadable != plan.coverage.selected.unreadable_headers.len() {
                if plan.global && record.record_path().is_ok() {
                    plan.grants.incomplete();
                } else {
                    plan.uncertain(record, input)?;
                }
            }
            Ok(())
        })?;
        if plan.has_rows && !plan.global {
            plan.reverse(forge, input, &wanted, &mut trees, cancel)?;
        }
        Ok(plan)
    }

    fn reverse(
        &mut self,
        forge: &'source DataForge<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        wanted: &Wanted,
        trees: &mut TreeBuilder<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        for record in forge.records() {
            checkpoint(cancel)?;
            if self.roots.contains(&record.index()) {
                continue;
            }
            let mut coverage = crate::catalog_queries::CatalogRecordCoverage::default();
            if !selected(&record, &mut coverage) {
                if !coverage.unreadable_headers.is_empty() {
                    self.uncertain(&record, input)?;
                    self.coverage
                        .dependencies
                        .unreadable_headers
                        .push(record.index());
                }
                continue;
            }
            if let Some((tree, survey)) = self.survey(&record, input, trees, cancel)? {
                let relevant = survey.relates_to(wanted);
                self.grants.merge(survey.grants.scope_only())?;
                if relevant {
                    self.coverage
                        .dependencies
                        .related_generators
                        .push(record.index());
                    self.generators.insert(record.index(), tree);
                }
            }
        }
        Ok(())
    }

    fn survey(
        &mut self,
        record: &Record<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        trees: &mut TreeBuilder<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<(RecordTree<'source>, Survey)>, CatalogError> {
        self.coverage
            .dependencies
            .surveyed_generators
            .push(record.index());
        let result = trees
            .build(record)
            .map_err(|error| -> ReadError { Box::new(error) })
            .and_then(|tree| Survey::read(&tree, input, cancel).map(|survey| (tree, survey)));
        match result {
            Ok(pair) => Ok(Some(pair)),
            Err(error) => {
                self.failed_survey(record, input, error)?;
                Ok(None)
            }
        }
    }

    pub(super) fn failed_survey(
        &mut self,
        record: &Record<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        error: ReadError,
    ) -> Result<(), CatalogError> {
        let error = match error.downcast::<CatalogError>() {
            Ok(error) => return Err(*error),
            Err(error) => error,
        };
        if self.global {
            self.grants.incomplete();
        } else {
            self.uncertain(record, input)?;
        }
        if self.roots.contains(&record.index()) {
            self.coverage
                .selected
                .unreadable_bodies
                .push(record.index());
        }
        self.coverage
            .dependencies
            .survey_failures
            .push(CatalogReadFailure::of(record, error.as_ref()));
        Ok(())
    }

    pub(super) fn uncertain(
        &mut self,
        record: &Record<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
    ) -> Result<(), CatalogError> {
        let context = input.record_context(record)?;
        let path_role = if record.record_path().is_ok() {
            SourceUseRole::Selector
        } else {
            SourceUseRole::Attempted
        };
        let type_role = if record.type_name().is_ok() {
            SourceUseRole::Selector
        } else {
            SourceUseRole::Attempted
        };
        self.grants
            .incomplete_record(&context, path_role, type_role)?;
        Ok(())
    }
}

pub(super) fn selected(
    record: &Record<'_>,
    coverage: &mut crate::catalog_queries::CatalogRecordCoverage,
) -> bool {
    let Ok(path) = record.record_path() else {
        if matches!(record.type_name(), Ok("ContractGenerator") | Err(_)) {
            coverage.unreadable_headers.push(record.index());
        }
        return false;
    };
    if !contract_generator_definition::matches_path(path, None) {
        return false;
    }
    match record.type_name() {
        Ok("ContractGenerator") => true,
        Ok(_) => false,
        Err(_) => {
            coverage.unreadable_headers.push(record.index());
            false
        }
    }
}
