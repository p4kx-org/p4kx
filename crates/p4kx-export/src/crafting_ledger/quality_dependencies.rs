// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{CancelToken, DataForge, Record, RecordTree, TreeBuilder};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::crafting_quantization::QUANTIZATIONS;
use crate::quality_catalog::contract::{QualityQuantizationResources, QuantizationEligibility};
use crate::quality_catalog::{Assembly, Input, Retention, Step, read, scan};
use crate::record_observation::RecordObservation;

use super::mode::{failed_quality, observe_selection};
use super::selection::{self, Action, ReadPolicy, Selection};

pub(crate) struct QualityDependencies {
    pub(crate) rows: Vec<QualityQuantizationResources>,
    pub(crate) coverage: CatalogRecordCoverage,
    pub(crate) dependency_coverage: CatalogRecordCoverage,
}

impl QualityDependencies {
    pub(crate) fn read<'source>(
        forge: &'source DataForge<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut requested = Requested::read(forge, selection, cancel)?;
        let mut survey = Survey::default();
        if !requested.owners.is_empty() {
            survey.read_candidates(forge, input, &mut requested, cancel)?;
        }
        Ok(Self {
            rows: survey.assembly.finish(input.origins(), cancel)?.inverse,
            coverage: requested.coverage,
            dependency_coverage: survey.coverage,
        })
    }
}

#[derive(Default)]
struct Requested {
    owners: HashSet<u32>,
    coverage: CatalogRecordCoverage,
}

impl Requested {
    fn read(
        forge: &DataForge<'_>,
        selected: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut requested = Self::default();
        selected.for_each(forge, cancel, |record| {
            let path = record.record_path();
            if path
                .as_ref()
                .is_ok_and(|path| !QUANTIZATIONS.matches_path(path))
            {
                return Ok(());
            }
            match selection::choose(record, None) {
                Ok(Some(selected)) if selected.action == Action::Quantization => {
                    if selected.policy == ReadPolicy::SkipAfter {
                        requested.coverage.skipped_leftovers += 1;
                    } else {
                        requested.owners.insert(record.index());
                    }
                }
                Err(_) => requested.coverage.unreadable_headers.push(record.index()),
                _ => {}
            }
            Ok(())
        })?;
        Ok(requested)
    }
}

#[derive(Default)]
struct Survey<'source> {
    assembly: Assembly<'source>,
    resources: Vec<Record<'source>>,
    selected_eligible: bool,
    coverage: CatalogRecordCoverage,
    trees: TreeBuilder<'source>,
}

impl<'source> Survey<'source> {
    fn read_candidates(
        &mut self,
        forge: &'source DataForge<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        requested: &mut Requested,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        for record in forge.records() {
            checkpoint(cancel)?;
            let (selected, inventory) = observe_selection(input, &record, None)?;
            let complete = inventory.complete;
            self.assembly.add(Step {
                inventory,
                input: None,
            })?;
            match selected {
                Ok(Some(candidate)) if candidate.action == Action::Quantization => {
                    self.quantizer(&record, input, candidate, requested, cancel)?;
                }
                Ok(Some(candidate)) if candidate.action == Action::ResourceLinks => {
                    self.resources.push(record);
                }
                Err(_) if !complete => self.coverage.unreadable_headers.push(record.index()),
                _ => {}
            }
        }
        if self.selected_eligible {
            self.read_resources(input, cancel)?;
        }
        Ok(())
    }

    fn quantizer(
        &mut self,
        record: &Record<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        candidate: Selection,
        requested: &mut Requested,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let selected = requested.owners.contains(&record.index());
        let coverage = if selected {
            &mut requested.coverage
        } else {
            &mut self.coverage
        };
        let Ok(tree) = self.trees.build(record) else {
            coverage.unreadable_bodies.push(record.index());
            return self
                .assembly
                .add(failed_quality(input, record, Some(candidate.action))?);
        };
        let admission = RecordObservation::capture(input, record)?;
        let eligibility =
            scan::inverse_candidate(&tree, &admission, input, candidate.role, cancel)?;
        if !eligibility.admission.complete || !eligibility.evidence.complete {
            coverage.incomplete_fields.push(record.index());
        }
        self.selected_eligible |=
            selected && matches!(eligibility.result, QuantizationEligibility::Eligible);
        let retention = if candidate.policy == ReadPolicy::SkipAfter {
            coverage.skipped_leftovers += 1;
            Retention::Leftover
        } else if selected {
            Retention::Selected
        } else {
            Retention::Dependency
        };
        self.assembly
            .retain_quantizer(read::owner(&tree), eligibility, retention);
        Ok(())
    }

    fn read_resources(
        &mut self,
        input: &OriginalCatalogInput<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        for record in std::mem::take(&mut self.resources) {
            checkpoint(cancel)?;
            let Ok(tree) = self.trees.build(&record) else {
                self.coverage.unreadable_bodies.push(record.index());
                self.assembly
                    .add(failed_quality(input, &record, Some(Action::ResourceLinks))?)?;
                continue;
            };
            self.resource(&record, &tree, input, cancel)?;
        }
        Ok(())
    }

    fn resource(
        &mut self,
        record: &Record<'source>,
        tree: &RecordTree<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let admission = RecordObservation::capture(input, record)?;
        let scanned = scan::resources(tree, input, &admission, cancel)?;
        if let Input::Resource(resource) = &scanned
            && !resource.inventory.complete
        {
            self.coverage.incomplete_fields.push(record.index());
        }
        self.assembly.add(Step {
            input: Some(scanned),
            ..Step::default()
        })
    }
}
