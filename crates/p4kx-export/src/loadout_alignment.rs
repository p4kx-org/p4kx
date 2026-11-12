// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{Attachment, PortNode};
use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::ledger::Capture;
use crate::CatalogError;
use crate::catalog_v3::catalog_item_alignment::{
    AlignmentIndex, AttachmentDefinitionLink, PortDefinitionLink,
};
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

#[derive(Debug, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LoadoutAlignment {
    port_definition: Option<PortDefinitionLink>,
    attached_item: Option<AttachedItem>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct AttachedItem {
    target_guid: SourcedValue<CatalogGuid>,
    definition: AttachmentDefinitionLink,
}

impl LoadoutAlignment {
    pub(crate) fn read(
        port: &PortNode,
        index: &AlignmentIndex<'_>,
        capture: &Capture<'_>,
    ) -> Result<Self, CatalogError> {
        let port_definition = port
            .definition()
            .map(|definition| index.port(definition.position()))
            .transpose()?;
        let attached_item = if let Attachment::Entity(child) = port.attachment()
            && let Some(record) = child.record_index()
        {
            Some(AttachedItem {
                target_guid: capture.attachment_target(child.guid(), child.attachment_target())?,
                definition: index.attachment(record, child.attachable_component_ordinal())?,
            })
        } else {
            None
        };
        Ok(Self {
            port_definition,
            attached_item,
        })
    }
}
