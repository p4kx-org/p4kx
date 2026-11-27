// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{CancelToken, Guid, Record, SourcedValue, TagTree, TreeBuilder};

use super::read;
use super::read::selection::{Action, SelectionError};
use super::{Piece, RewardDependency};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalReferenceObservation, RewardRead};
use crate::resource_integer::ResourceInt64;
use crate::{CatalogError, CatalogReadFailure, ExportAccountingError};

/// Observes only the original physical rewards reached by selected Broker rows.
pub(crate) struct BrokerRewardReads<'source> {
    trees: TreeBuilder<'source>,
    reads: BTreeMap<(Guid, u32), RewardRead>,
    failures: Vec<CatalogReadFailure<'source>>,
    incomplete_fields: Vec<u32>,
}

impl<'source> BrokerRewardReads<'source> {
    pub(crate) fn new() -> Self {
        Self {
            trees: TreeBuilder::new(),
            reads: BTreeMap::new(),
            failures: Vec::new(),
            incomplete_fields: Vec::new(),
        }
    }

    pub(crate) fn retain(
        &mut self,
        input: &OriginalCatalogInput<'_, 'source>,
        reference: &OriginalReferenceObservation<'_, '_, '_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<SourcedValue<ResourceInt64>>, CatalogError> {
        reference.caller().check_input(input)?;
        if reference.is_instance() {
            return Ok(None);
        }
        let record = reference.record()?;
        input.record_context(&record)?;
        match read::selection::choose_dependency(&record) {
            Some(Ok(Action::Reward)) => {}
            Some(Err(SelectionError::Path(_))) => {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            Some(Err(SelectionError::SourceType(error))) => {
                return Err(CatalogError::DataForge(error));
            }
            _ => return Ok(None),
        }
        let key = (record.guid(), record.index());
        let reward = match self.reads.entry(key) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => entry.insert(read_reward(
                &mut self.trees,
                input,
                &record,
                cancel,
                &mut self.failures,
                &mut self.incomplete_fields,
            )?),
        };
        reward.retain(input, reference).map(Some)
    }

    pub(crate) fn finish(self) -> (Vec<CatalogReadFailure<'source>>, Vec<u32>) {
        (self.failures, self.incomplete_fields)
    }
}

fn read_reward<'source>(
    trees: &mut TreeBuilder<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    record: &Record<'source>,
    cancel: Option<&CancelToken>,
    failures: &mut Vec<CatalogReadFailure<'source>>,
    incomplete_fields: &mut Vec<u32>,
) -> Result<RewardRead, CatalogError> {
    let observed = read::read(
        trees,
        record,
        &TagTree::default(),
        Some(input),
        None,
        cancel,
    )?
    .ok_or(ExportAccountingError::UnobservedFailure)?;
    if !observed.ledger.field_projection_complete(0, 0) {
        incomplete_fields.push(record.index());
    }
    match observed.piece {
        Some(Piece::Reward(guid, index, RewardDependency::Original(reward)))
            if (guid, index) == (record.guid(), record.index()) =>
        {
            Ok(reward)
        }
        Some(Piece::UnusableRewardCandidate(failure, RewardDependency::Original(reward)))
            if failure.record().index() == record.index() =>
        {
            failures.push(failure);
            Ok(reward)
        }
        _ => Err(ExportAccountingError::UnobservedFailure.into()),
    }
}
