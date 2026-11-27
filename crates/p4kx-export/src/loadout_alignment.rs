// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{Attachment, PortDefinitionPosition, PortNode};
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

#[path = "loadout_alignment_direct.rs"]
pub(crate) mod direct;

/// Resolves the canonical links used by a Loadout port.
pub(crate) trait AlignmentLookup {
    fn port(
        &mut self,
        position: PortDefinitionPosition,
    ) -> Result<PortDefinitionLink, CatalogError>;

    fn attachment(
        &mut self,
        record: u32,
        component: Option<usize>,
    ) -> Result<AttachmentDefinitionLink, CatalogError>;
}

impl AlignmentLookup for AlignmentIndex<'_> {
    fn port(
        &mut self,
        position: PortDefinitionPosition,
    ) -> Result<PortDefinitionLink, CatalogError> {
        AlignmentIndex::port(self, position)
    }

    fn attachment(
        &mut self,
        record: u32,
        component: Option<usize>,
    ) -> Result<AttachmentDefinitionLink, CatalogError> {
        AlignmentIndex::attachment(self, record, component)
    }
}

#[derive(Debug, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// The physical source links used by the resolved port.
pub struct LoadoutAlignment {
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
pub struct AttachmentIdentity {
    pub(crate) target_guid: SourcedValue<CatalogGuid>,
    pub(crate) target_record_index: SourcedValue<u32>,
}

#[derive(Debug)]
pub(crate) struct AttachmentProof {
    pub(crate) source_uses: Vec<SourceUse>,
    pub(crate) source_set_complete: bool,
}

impl AttachmentIdentity {
    /// Borrows the GUID of the actually resolved attached Entity.
    #[must_use]
    pub const fn target_guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.target_guid
    }

    /// Borrows the exact physical attached Record position.
    #[must_use]
    pub const fn target_record_index(&self) -> &SourcedValue<u32> {
        &self.target_record_index
    }
}

impl LoadoutAlignment {
    /// Borrows the source link for the resolved port definition.
    #[must_use]
    pub const fn port_definition(&self) -> Option<&PortDefinitionLink> {
        self.port_definition.as_ref()
    }

    /// Borrows the configuration link for the actually attached Entity.
    #[must_use]
    pub fn attachment_definition(&self) -> Option<&AttachmentDefinitionLink> {
        self.attached_item.as_ref().map(|item| &item.definition)
    }
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

    /// Borrows the sourced identity of the actually attached Entity.
    #[must_use]
    pub fn attachment(&self) -> Option<&AttachmentIdentity> {
        self.attached_item
            .as_ref()
            .map(|attachment| &attachment.identity)
    }

    pub(crate) fn read(
        port: &PortNode,
        index: &mut (impl AlignmentLookup + ?Sized),
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
