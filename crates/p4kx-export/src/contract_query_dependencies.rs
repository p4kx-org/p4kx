// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{CancelToken, DataForge, DataForgeError, Guid, RecordTree, TreeBuilder};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::contract_starmap::StarmapObject;
use crate::mission_index::demand::{DependencyKey, PreparedDependencies, PreparedOutcome};
use crate::mission_index::read::selection::{self, Action, SelectionError};
use crate::mission_index::{MissionIndex, read};
use crate::record_read::{self, RecordAdmission};
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "contract_query_place_rows.rs"]
mod place_rows;

pub(super) struct Inputs<'context, 'source> {
    pub(super) original: &'context OriginalCatalogInput<'context, 'source>,
    pub(super) tags: &'context CapturedTags<'context, 'source>,
    pub(super) places: &'context CatalogProjection<'context, 'source>,
    pub(super) cancel: Option<&'context CancelToken>,
}

/// Original candidates read only in response to canonical typed lookups.
pub(super) struct Dependencies<'source> {
    forge: &'source DataForge<'source>,
    by_guid: BTreeMap<Guid, Vec<u32>>,
    trees: TreeBuilder<'source>,
    records: BTreeMap<u32, Result<RecordTree<'source>, DataForgeError>>,
    objects: BTreeMap<u32, StarmapObject<'source>>,
    indexed: BTreeSet<u32>,
    types: crate::starmap::TypeNames<'source>,
    pub(super) prepared: PreparedDependencies,
}

impl<'source> Dependencies<'source> {
    pub(super) fn new(
        forge: &'source DataForge<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut by_guid: BTreeMap<Guid, Vec<u32>> = BTreeMap::new();
        for record in forge.records() {
            checkpoint(cancel)?;
            by_guid
                .entry(record.guid())
                .or_default()
                .push(record.index());
        }
        Ok(Self {
            forge,
            by_guid,
            trees: TreeBuilder::new(),
            records: BTreeMap::new(),
            objects: BTreeMap::new(),
            indexed: BTreeSet::new(),
            types: crate::starmap::TypeNames::default(),
            prepared: PreparedDependencies::query(),
        })
    }

    pub(super) fn objects(&self) -> impl Iterator<Item = &StarmapObject<'source>> {
        self.objects.values()
    }

    pub(super) fn prepare(
        &mut self,
        key: DependencyKey,
        input: &Inputs<'_, 'source>,
        index: &mut MissionIndex<'source>,
        coverage: &mut super::ContractQueryCoverage<'source>,
    ) -> Result<(), CatalogError> {
        let (guid, physical) = match key {
            DependencyKey::Mission(_, guid) | DependencyKey::Place(guid) => (guid, None),
            DependencyKey::MissionItem { guid, record_index }
            | DependencyKey::MissionLocality { guid, record_index } => (guid, Some(record_index)),
        };
        let count = physical.map_or_else(|| self.by_guid.get(&guid).map_or(0, Vec::len), |_| 1);
        let mut admitted = false;
        let mut failed = false;
        for slot in 0..count {
            checkpoint(input.cancel)?;
            let position = if let Some(position) = physical {
                position
            } else {
                *self
                    .by_guid
                    .get(&guid)
                    .and_then(|rows| rows.get(slot))
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?
            };
            let Some(record) = self.forge.record(position) else {
                continue;
            };
            if record.guid() != guid {
                continue;
            }
            let Some(candidate) = candidate(&record, key) else {
                continue;
            };
            admitted = true;
            failed |= self.read(position, candidate, input, index, coverage)?;
        }
        coverage.dependencies.forward_records.sort_unstable();
        coverage.dependencies.forward_records.dedup();
        self.prepared.prepare(
            key,
            if failed {
                PreparedOutcome::Failed
            } else if admitted {
                PreparedOutcome::Proven
            } else {
                PreparedOutcome::Absent
            },
        )
    }

    fn read(
        &mut self,
        position: u32,
        candidate: Candidate,
        input: &Inputs<'_, 'source>,
        index: &mut MissionIndex<'source>,
        coverage: &mut super::ContractQueryCoverage<'source>,
    ) -> Result<bool, CatalogError> {
        let record = self
            .forge
            .record(position)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let is_place = matches!(&candidate, Candidate::Place(_));
        if !self.records.contains_key(&position) {
            coverage.dependencies.forward_records.push(position);
            let admission = match candidate {
                Candidate::Mission(admission) | Candidate::Place(admission) => admission,
            };
            let tree = match admission {
                Ok(()) => self.trees.build(&record),
                Err(error) => Err(error),
            };
            if let Err(error) = &tree {
                coverage
                    .dependencies
                    .forward_failures
                    .push(crate::CatalogReadFailure::of(&record, error));
            }
            self.records.insert(position, tree);
        }
        let tree = self
            .records
            .get(&position)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let failed = tree.is_err();
        if is_place {
            if let Ok(tree) = tree
                && !self.objects.contains_key(&position)
            {
                let (object, diagnostics) = place_rows::read(
                    self.forge,
                    &mut self.trees,
                    tree,
                    &self.types,
                    input.places,
                    input.cancel,
                )?;
                coverage.diagnostics.extend(diagnostics);
                self.objects.insert(position, object);
            }
            Ok(failed)
        } else if self.indexed.insert(position) {
            let cached = match tree {
                Ok(tree) => read::CachedTree::Read(tree),
                Err(error) => read::CachedTree::Failed(error),
            };
            Ok(index.extend_cached_record(
                input.original,
                input.tags,
                record,
                cached,
                input.cancel,
            )? || failed)
        } else {
            Ok(failed)
        }
    }

    pub(super) fn final_objects(
        &mut self,
        consulted: &BTreeSet<DependencyKey>,
        projection: &CatalogProjection<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Vec<StarmapObject<'source>>, CatalogError> {
        let mut objects = Vec::new();
        for (&position, object) in &self.objects {
            if !consulted.contains(&DependencyKey::Place(object.guid.0)) {
                continue;
            }
            let tree = self
                .records
                .get(&position)
                .and_then(|tree| tree.as_ref().ok())
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            let (object, _) = place_rows::read(
                self.forge,
                &mut self.trees,
                tree,
                &self.types,
                projection,
                cancel,
            )?;
            objects.push(object);
        }
        Ok(objects)
    }
}

enum Candidate {
    Mission(Result<(), DataForgeError>),
    Place(Result<(), DataForgeError>),
}

fn candidate(record: &p4kx_extraction::Record<'_>, key: DependencyKey) -> Option<Candidate> {
    match key {
        DependencyKey::Place(_) => match record_read::classify(record, Kind::StarmapObjects) {
            Ok(RecordAdmission::Selected) => Some(Candidate::Place(Ok(()))),
            Ok(RecordAdmission::Other | RecordAdmission::Leftover) => None,
            Err(error) => Some(Candidate::Place(Err(error))),
        },
        DependencyKey::Mission(expected, _) => mission(record, expected),
        DependencyKey::MissionItem { .. } => mission(record, Action::Item),
        DependencyKey::MissionLocality { .. } => mission(record, Action::Locality),
    }
}

fn mission(record: &p4kx_extraction::Record<'_>, expected: Action) -> Option<Candidate> {
    match selection::choose_dependency(record)? {
        Ok(actual) if actual == expected => Some(Candidate::Mission(Ok(()))),
        Ok(_) => None,
        Err(SelectionError::Path(error) | SelectionError::SourceType(error)) => {
            Some(Candidate::Mission(Err(error)))
        }
    }
}
