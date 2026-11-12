// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{Attachment, PortNode};
use p4kx_extraction::{SourceUse, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::ledger::Capture;
use crate::CatalogError;
use crate::catalog_v3::catalog_item_alignment::{
    AlignmentIndex, AttachmentDefinitionLink, PhysicalPort, PortDefinitionLink,
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
    #[serde(flatten)]
    identity: AttachmentIdentity,
    definition: AttachmentDefinitionLink,
    #[serde(skip)]
    proof: AttachmentProof,
}

/// The exact attached Record selected by the resolved Loadout.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AttachmentIdentity {
    pub(crate) target_guid: SourcedValue<CatalogGuid>,
    pub(crate) target_record_index: SourcedValue<u32>,
}

#[derive(Debug)]
pub(crate) struct AttachmentProof {
    pub(crate) source_uses: Vec<SourceUse>,
    pub(crate) source_set_complete: bool,
}

impl LoadoutAlignment {
    pub(crate) fn physical_port(&self) -> Result<Option<PhysicalPort>, CatalogError> {
        self.port_definition
            .as_ref()
            .map_or(Ok(None), PortDefinitionLink::physical_port)
    }

    pub(crate) fn attachment_component(&self) -> Result<Option<(u32, u32)>, CatalogError> {
        self.attached_item
            .as_ref()
            .map_or(Ok(None), |item| item.definition.physical_key())
    }

    pub(crate) fn attachment_proof(&self) -> Option<&AttachmentProof> {
        self.attached_item
            .as_ref()
            .map(|attachment| &attachment.proof)
    }

    pub(crate) fn placement_attachment(&self) -> Option<(&AttachmentIdentity, &AttachmentProof)> {
        self.attached_item
            .as_ref()
            .map(|attachment| (&attachment.identity, &attachment.proof))
    }

    pub(crate) fn attachment(&self) -> Option<&AttachmentIdentity> {
        self.attached_item
            .as_ref()
            .map(|attachment| &attachment.identity)
    }

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
            let (identity, proof) =
                capture.attachment_identity(child.guid(), record, child.attachment_target())?;
            Some(AttachedItem {
                identity,
                proof,
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
