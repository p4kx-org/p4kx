// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::SourceElement;

use super::super::clip_candidates::Fact;
use super::super::event_bindings;
use super::super::payload::LinkKind;
use super::super::projection::DefinitionProjection;
use super::LinkSelector;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{Identity, MediaError, MediaWriter, Reference, ReferenceResolution, origin_error};

pub(super) fn unique_combined_path(element: SourceElement<'_>) -> bool {
    let mut paths = element.attributes().filter(|attribute| {
        attribute.namespace().is_none() && attribute.name().eq_ignore_ascii_case("AName")
    });
    paths.next().is_some() && paths.next().is_none()
}

impl LinkSelector {
    pub(super) fn observe_contributor_arc(
        &self,
        projection: &DefinitionProjection<'_, '_>,
        reference: &Reference,
        prepared: &PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let directory = projection.context.directory;
        if projection.repaired_strings || !directory.uses_definition_scope() {
            return Ok(());
        }
        let (Some(kind), ReferenceResolution::Resolved { target }) =
            (self.contributor_arc, reference.resolution())
        else {
            return Ok(());
        };
        let Identity::Entry { entry, .. } = &target.source else {
            return Err(origin_error::foreign());
        };
        let mut proof = writer.capture_origins();
        prepared.include_known("/data/payload/reference", &mut proof)?;
        directory.index.animation_clip_candidates.observe(
            Fact::Arc {
                from: projection.context.entry.index(),
                to: entry.entry_index,
                kind,
                proof: &proof.finish(),
            },
            writer,
        )
    }

    pub(super) fn observe_event_database(
        &self,
        projection: &DefinitionProjection<'_, '_>,
        indices: [u64; 2],
        reference: &Reference,
        prepared: &PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let Some(list) = self
            .character_list
            .filter(|_| matches!(self.kind, LinkKind::EventDatabase))
        else {
            return Ok(());
        };
        projection
            .context
            .directory
            .index
            .animation_event_bindings
            .database(
                event_bindings::List {
                    entry: projection.context.entry.index(),
                    element: list,
                },
                indices,
                reference,
                prepared,
                writer,
            )
    }
}
