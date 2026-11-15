// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{EntityRig, RigPlacement};
use p4kx_extraction::{DataForge, Entry, InputScope, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;

use super::adb_entity_selectors::{self, Selector};
use super::clip_candidates::Fact;
use super::definition_scope::{allocation, selected_target};
use super::links::{self, ReferenceSelection};
use super::origins;
use super::payload::{self as wire, DefinitionPayload, LinkKind};
use super::scope_placement::PlacementEvidence;
use crate::animation::entity_loadout_evidence::EntityLoadoutEvidence;
use crate::audio_record_origins as records;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{
    DirectoryLookup, Identity, MediaError, MediaKind, MediaWriter, Reference, ReferenceResolution,
    Status, Subresource,
};

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct EntityDatabaseBinding<'source> {
    owner_record_index: u32,
    component_index: usize,
    field: &'static str,
    authored_path: &'source str,
    archive_path: Option<&'source str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    port_path: Option<&'source str>,
    #[schemars(schema_with = "crate::schema::reference_definition")]
    reference: Reference,
}

pub(super) struct Bindings {
    owner: MediaOriginInput,
    rows: Vec<Binding>,
}

struct Binding {
    record_index: u32,
    rig_ordinal: u64,
    component: usize,
    field: &'static str,
    authored_path: String,
    archive_path: Option<String>,
    original: OriginContributions,
    owner: OriginContributions,
    port: Option<Port>,
}

struct Port {
    path: String,
    original: OriginContributions,
}

impl Bindings {
    pub(super) fn collect(
        forge: &DataForge<'_>,
        rigs: &[EntityRig],
        loadout: &EntityLoadoutEvidence,
        entry: &Entry,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let owner = writer
            .origins
            .directory()
            .cloned()
            .ok_or_else(crate::origin_error::foreign)?;
        let input = writer.origins.input(lookup.index.archive, entry)?;
        let mut rows = Vec::new();
        adb_entity_selectors::visit(forge, rigs, lookup, writer, |selector, writer| {
            rows.try_reserve(1)
                .map_err(|cause| allocation(writer, cause))?;
            rows.push(Binding::capture(&selector, &input, loadout, writer)?);
            Ok(())
        })?;
        Ok(Self { owner, rows })
    }

    pub(super) fn roots(
        &self,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Vec<usize>, MediaError> {
        let mut roots = Vec::new();
        for row in &self.rows {
            writer.checkpoint()?;
            if let Some(path) = &row.archive_path
                && let Some(entry) = selected_target(path, lookup, writer)?
            {
                roots
                    .try_reserve(1)
                    .map_err(|cause| allocation(writer, cause))?;
                roots.push(entry.index());
            }
        }
        Ok(roots)
    }

    pub(super) fn write(
        self,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        for (ordinal, row) in self.rows.iter().enumerate() {
            row.write(ordinal as u64, &self.owner, lookup, writer)?;
        }
        Ok(())
    }
}

impl Binding {
    fn capture(
        selector: &Selector<'_>,
        input: &MediaOriginInput,
        loadout: &EntityLoadoutEvidence,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let coordinates = [u64::from(selector.record_index)];
        let field = format!("Components[{}].{}", selector.component, selector.field);
        let mut original = writer.capture_origins();
        original.add(
            input,
            InputScope::Payload,
            OriginSite::new("dataforge-record", &coordinates, &field),
            SourceUseRole::Selector,
            writer,
        )?;
        let placement = PlacementEvidence::new(selector.rig, input, loadout, writer)?;
        let mut owner = writer.capture_origins();
        owner.include_original(&placement.owner)?;
        owner.add(
            input,
            InputScope::Payload,
            OriginSite::new("dataforge-record", &coordinates, "record_index"),
            SourceUseRole::Selector,
            writer,
        )?;
        let port = match &selector.rig.placement {
            RigPlacement::Attachment { port_path } => Some(Port {
                path: port_path.clone(),
                original: placement.route(writer)?,
            }),
            _ => None,
        };
        Ok(Self {
            record_index: selector.record_index,
            rig_ordinal: selector.rig_ordinal,
            component: selector.component,
            field: selector.field,
            authored_path: selector.authored.to_owned(),
            archive_path: links::target_path(LinkKind::SubDatabase, selector.authored.trim()),
            original: original.finish(),
            owner: owner.finish(),
            port,
        })
    }

    fn write(
        &self,
        ordinal: u64,
        input: &MediaOriginInput,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        writer.checkpoint()?;
        let mut prepared = origins::record(input, self.original.clone(), writer)?;
        records::field(
            &mut prepared,
            "/data/payload/owner_record_index",
            self.owner.clone(),
            writer,
        )?;
        if let Some(port) = &self.port {
            records::field(
                &mut prepared,
                "/data/payload/port_path",
                port.original.clone(),
                writer,
            )?;
        }
        let reference = ReferenceSelection {
            authored_path: &self.authored_path,
            archive_path: self.archive_path.as_deref(),
            kind: Some(MediaKind::AnimationDefinition),
            original: &self.original,
            selector: &self.original,
        }
        .resolve_at(lookup, "/data/payload/reference", &mut prepared, writer)?;
        self.observe(ordinal, lookup, &reference, &prepared, writer)?;
        prepared.write(
            writer,
            &Subresource::new(wire::ENTITY_DATABASE_BINDING, vec![ordinal]),
            Status::Complete,
            &DefinitionPayload::EntityDatabaseBinding(EntityDatabaseBinding {
                owner_record_index: self.record_index,
                component_index: self.component,
                field: self.field,
                authored_path: &self.authored_path,
                archive_path: self.archive_path.as_deref(),
                port_path: self.port.as_ref().map(|port| port.path.as_str()),
                reference,
            }),
            &[],
        )
    }

    fn observe(
        &self,
        ordinal: u64,
        lookup: &DirectoryLookup<'_>,
        reference: &Reference,
        prepared: &crate::origin_prepared::PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let ReferenceResolution::Resolved { target } = reference.resolution() else {
            return Ok(());
        };
        let Identity::Entry { entry, .. } = &target.source else {
            return Err(crate::origin_error::foreign());
        };
        let mut proof = writer.capture_origins();
        prepared.include_known("/data/payload/reference", &mut proof)?;
        proof.include_original(&self.owner)?;
        if let Some(port) = &self.port {
            proof.include_original(&port.original)?;
        }
        lookup.index.animation_clip_candidates.observe(
            Fact::Binding {
                ordinal,
                placement: self.rig_ordinal,
                root: entry.entry_index,
                proof: &proof.finish(),
            },
            writer,
        )
    }
}
