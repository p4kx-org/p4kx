// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Record};

use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session;
use crate::commodity_backfill;
use crate::entity_selection::{self, Selected};
use crate::{CatalogError, Kind};

pub(super) struct Plan<'source> {
    pub(super) entities: Vec<Record<'source>>,
    pub(super) resources: Vec<Record<'source>>,
    pub(super) coverage: CatalogRecordCoverage,
    pub(super) representation_complete: bool,
}

impl<'source> Plan<'source> {
    pub(super) fn read(
        forge: &'source DataForge<'_>,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut plan = Self {
            entities: Vec::new(),
            resources: Vec::new(),
            coverage: CatalogRecordCoverage::default(),
            representation_complete: true,
        };
        selection.for_each(forge, cancel, |record| {
            catalog_query_session::checkpoint(cancel)?;
            match entity_selection::select(record, None) {
                Ok(Some(Selected::Entity(Kind::Commodities))) => plan.entities.push(*record),
                Ok(Some(Selected::Entity(Kind::ResourceTypes))) => plan.resources.push(*record),
                Ok(Some(Selected::Leftover(Kind::Commodities | Kind::ResourceTypes))) => {
                    plan.coverage.skipped_leftovers += 1;
                }
                Ok(_) => {}
                Err(_) => {
                    plan.coverage.unreadable_headers.push(record.index());
                    plan.representation_complete &=
                        !commodity_backfill::header_failure_affects_presence(record);
                }
            }
            Ok(())
        })?;
        Ok(plan)
    }
}
