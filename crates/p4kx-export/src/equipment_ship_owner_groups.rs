// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::vec::IntoIter;

use p4kx_extraction::{CancelToken, DataForge, Record, TreeBuilder};

use crate::build::catalog_queries::unlocalized_record_query::{self, plan};
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::guid_text::CatalogGuid;
use crate::{CatalogError, Kind};

use plan::{OrderedOwnerPlan, Owner, Plan};

const OWNER_GROUP_SIZE: usize = 32;

pub(super) struct OwnerGroups<'source> {
    pending: PendingOwners<'source>,
    trees: TreeBuilder<'source>,
    pub(super) coverage: CatalogRecordCoverage,
}

enum PendingOwners<'source> {
    Prepared(Vec<Owner<'source>>),
    Records(IntoIter<Record<'source>>),
}

impl<'source> OwnerGroups<'source> {
    pub(super) fn all(
        forge: &'source DataForge<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let (records, coverage) = OrderedOwnerPlan::select(
            forge,
            CatalogRecordSelection::All,
            cancel,
            |record, coverage| unlocalized_record_query::selected(record, Kind::Ships, coverage),
        )?
        .into_parts();
        Ok(Self {
            pending: PendingOwners::Records(records.into_iter()),
            trees: TreeBuilder::new(),
            coverage,
        })
    }

    pub(super) fn prepared(mut plan: Plan<'source>) -> Self {
        plan.owners.sort_by_cached_key(|owner| {
            (
                CatalogGuid(owner.record.guid()).text(),
                owner.record.index(),
            )
        });
        Self {
            pending: PendingOwners::Prepared(plan.owners),
            trees: TreeBuilder::new(),
            coverage: plan.coverage,
        }
    }

    pub(super) const fn is_all(&self) -> bool {
        matches!(&self.pending, PendingOwners::Records(_))
    }

    pub(super) fn fill(
        &mut self,
        owners: &mut Vec<Owner<'source>>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        match &mut self.pending {
            PendingOwners::Prepared(prepared) => std::mem::swap(owners, prepared),
            PendingOwners::Records(records) => {
                while owners.is_empty() && !records.as_slice().is_empty() {
                    for record in records.by_ref().take(OWNER_GROUP_SIZE) {
                        checkpoint(cancel)?;
                        if let Some(owner) =
                            plan::read_owner(&mut self.trees, &record, &mut self.coverage)?
                        {
                            owners.push(owner);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn finish(mut self) -> CatalogRecordCoverage {
        self.coverage.unreadable_bodies.sort_unstable();
        self.coverage
    }
}
