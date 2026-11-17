// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourcedValue, Step};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::OriginalInstanceLocation;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::definition_reference::DefinitionStatus;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

/// A stored Reference whose retained target text borrows the original admitted input.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRecordReference<'source> {
    pub(crate) target_record_index: SourcedValue<u32>,
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) record_path: SourcedValue<&'source str>,
    pub(crate) target_type: SourcedValue<&'source str>,
    pub(crate) status: DefinitionStatus,
}

impl<'source> SourceRecordReference<'source> {
    /// The original referenced GUID with its field evidence.
    #[must_use]
    pub fn guid(&self) -> SourcedValue<Guid> {
        self.guid.map_ref(|guid| guid.0)
    }

    /// The physical target Record index when available.
    #[must_use]
    pub const fn target_record_index(&self) -> &SourcedValue<u32> {
        &self.target_record_index
    }

    /// The original target Record path when available.
    #[must_use]
    pub const fn record_path(&self) -> &SourcedValue<&'source str> {
        &self.record_path
    }

    /// The original target Record type when available.
    #[must_use]
    pub const fn target_type(&self) -> &SourcedValue<&'source str> {
        &self.target_type
    }

    /// The source resolution state of the original reference.
    #[must_use]
    pub const fn status(&self) -> &DefinitionStatus {
        &self.status
    }

    /// Owns the original target text while retaining its existing field origins.
    pub(crate) fn into_catalog(self) -> CatalogRecordReference {
        CatalogRecordReference {
            target_record_index: self.target_record_index,
            guid: self.guid,
            record_path: self.record_path.map(str::to_owned),
            target_type: self.target_type.map(str::to_owned),
            status: self.status,
        }
    }
    /// Returns the physical Record index for a resolved compatible target.
    #[must_use]
    pub const fn resolved_index(&self) -> Option<u32> {
        if matches!(self.status, DefinitionStatus::Present) {
            self.target_record_index.value().copied()
        } else {
            None
        }
    }
}

/// A pointer-derived admitted target with independently sourced original facts.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SourceWeakReference<'source> {
    pub(crate) target_record_index: SourcedValue<u32>,
    pub(crate) record_guid: SourcedValue<CatalogGuid>,
    pub(crate) record_path: SourcedValue<&'source str>,
    pub(crate) target: SourceWeakTarget<'source>,
}

impl<'source> SourceWeakReference<'source> {
    pub(crate) fn instance_steps(&self) -> Option<&[Step<'source>]> {
        match &self.target {
            SourceWeakTarget::Instance { location, .. } => Some(location.value()?.steps()),
            SourceWeakTarget::Record { .. } => None,
        }
    }
    /// Returns a Record link when the pointer targeted a Record header.
    pub(crate) fn into_record(self) -> Option<SourceRecordReference<'source>> {
        let SourceWeakTarget::Record { source_type } = self.target else {
            return None;
        };
        Some(SourceRecordReference {
            target_record_index: self.target_record_index,
            guid: self.record_guid,
            record_path: self.record_path,
            target_type: source_type,
            status: DefinitionStatus::Present,
        })
    }
}

/// Actual Record versus owned-instance applicability from the original admission.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum SourceWeakTarget<'source> {
    Record {
        source_type: SourcedValue<&'source str>,
    },
    Instance {
        source_type: SourcedValue<&'source str>,
        #[schemars(with = "SourcedValue<String>")]
        location: SourcedValue<OriginalInstanceLocation<'source>>,
    },
}
