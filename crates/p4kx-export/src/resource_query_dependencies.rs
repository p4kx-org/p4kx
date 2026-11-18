// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{CancelToken, CanonicalReadObserver, Guid, TreeBuilder};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_query_session::checkpoint;
use crate::entity_selection::{self, Selected};
use crate::record_observation::RecordObservation;
use crate::resource_group::{self, Groups, ResourceMemberships};
use crate::{CatalogError, Kind};

/// A `ResourceType`'s reverse membership dependency prepared once on canonical demand.
#[derive(Default)]
pub(super) struct Dependencies {
    groups: Option<Groups>,
    observer: Option<Arc<dyn CanonicalReadObserver>>,
    pub(super) coverage: CatalogRecordCoverage,
}

impl Dependencies {
    pub(super) fn new(observer: Option<Arc<dyn CanonicalReadObserver>>) -> Self {
        Self {
            observer,
            ..Self::default()
        }
    }

    pub(super) fn memberships(
        &mut self,
        resource: Guid,
        input: &OriginalCatalogInput<'_, '_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ResourceMemberships, CatalogError> {
        if let Some(groups) = &self.groups {
            return Ok(groups.all_of_resource(&resource));
        }
        let groups = self.prepare(input, cancel)?;
        let memberships = groups.all_of_resource(&resource);
        self.groups = Some(groups);
        Ok(memberships)
    }

    fn prepare(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Groups, CatalogError> {
        let mut groups = Groups::default();
        let mut trees = TreeBuilder::new();
        trees.set_read_observer(self.observer.clone());
        for record in input.forge().records() {
            checkpoint(cancel)?;
            match entity_selection::select(&record, None) {
                Ok(Some(Selected::Entity(Kind::ResourceGroups))) => {}
                Ok(Some(Selected::Leftover(Kind::ResourceGroups))) => {
                    self.coverage.skipped_leftovers += 1;
                    continue;
                }
                Ok(_) => continue,
                Err(_) => {
                    self.coverage.unreadable_headers.push(record.index());
                    groups.mark_incomplete();
                    continue;
                }
            }
            let admission = RecordObservation::capture(input, &record)?;
            if let Ok(tree) = trees.build(&record) {
                input.record_scope(admission.record(), &tree)?;
                let listing = resource_group::listing(&tree, input.origins(), cancel)?;
                if !listing.resources_complete() {
                    self.coverage.incomplete_fields.push(record.index());
                }
                groups.add(listing, cancel)?;
            } else {
                self.coverage.unreadable_bodies.push(record.index());
                groups.mark_incomplete();
            }
        }
        Ok(groups)
    }
}
