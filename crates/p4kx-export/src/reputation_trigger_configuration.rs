// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceValue, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::sandbox_trigger::SandboxTrigger;
use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Original sandbox trigger occurrences owned by one physical faction reputation Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ReputationTriggerConfiguration<'source> {
    owner: ReputationTriggerOwner,
    sandbox_triggers: SourcedValue<Vec<SourcedValue<SandboxTrigger<'source>>>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ReputationTriggerOwner {
    guid: SourcedValue<CatalogGuid>,
    record_index: SourcedValue<u32>,
}

impl<'source> ReputationTriggerConfiguration<'source> {
    /// Reads the owned array and its physical Record identity.
    pub(super) fn read(
        tree: &RecordTree<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        origins: &CatalogOrigins,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let context = origins.record(tree.record_index());
        let owner = ReputationTriggerOwner {
            guid: context.header_value(
                RecordHeaderField::Guid,
                SourceValue::Present(CatalogGuid(tree.guid())),
            )?,
            record_index: context.header_selector(
                RecordHeaderField::RecordIndex,
                SourceValue::Present(tree.record_index()),
            )?,
        };
        Ok(Self {
            owner,
            sandbox_triggers: SandboxTrigger::read_array(fields, notes)?,
        })
    }

    /// Returns the source state used to decide whether raw fallback is needed.
    pub(super) const fn state(&self) -> SourceValueState {
        self.sandbox_triggers.state()
    }
}
