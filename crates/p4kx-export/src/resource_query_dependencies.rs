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
use crate::scan_signature::{RADAR_GLOBAL_PARAMS, SignatureSlot};
use crate::{CatalogError, Kind};

/// A `ResourceType`'s reverse membership dependency prepared once on canonical demand.
#[derive(Default)]
pub(in crate::build::catalog_queries) struct Dependencies {
    groups: Option<Groups>,
    signature_slot: Option<SignatureSlot>,
    observer: Option<Arc<dyn CanonicalReadObserver>>,
    pub(in crate::build::catalog_queries) coverage: CatalogRecordCoverage,
}

impl Dependencies {
    pub(in crate::build::catalog_queries) fn new(
        observer: Option<Arc<dyn CanonicalReadObserver>>,
    ) -> Self {
        Self {
            observer,
            ..Self::default()
        }
    }

    pub(super) fn signature_slot(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        cancel: Option<&CancelToken>,
    ) -> Result<&SignatureSlot, CatalogError> {
        if self.signature_slot.is_none() {
            let mut records = Vec::new();
            for record in input.forge().records() {
                checkpoint(cancel)?;
                if record.type_name().ok() == Some(RADAR_GLOBAL_PARAMS) {
                    records.push(record);
                }
            }
            let (slot, failures, _, _) = SignatureSlot::read_captured(&records, input, cancel)?;
            self.coverage
                .unreadable_bodies
                .extend(failures.iter().map(|failure| failure.record().index()));
            return Ok(self.signature_slot.insert(slot));
        }
        self.signature_slot
            .as_ref()
            .ok_or_else(|| crate::ExportAccountingError::ConflictingSourceDatum.into())
    }

    pub(super) fn memberships(
        &mut self,
        resource: Guid,
        input: &OriginalCatalogInput<'_, '_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ResourceMemberships, CatalogError> {
        Ok(self.groups(input, cancel)?.0.all_of_resource(&resource))
    }

    pub(in crate::build::catalog_queries) fn groups(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        cancel: Option<&CancelToken>,
    ) -> Result<(&Groups, &CatalogRecordCoverage), CatalogError> {
        if self.groups.is_none() {
            self.groups = Some(self.prepare(input, cancel)?);
        }
        let groups = self
            .groups
            .as_ref()
            .ok_or(crate::ExportAccountingError::ConflictingSourceDatum)?;
        Ok((groups, &self.coverage))
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
