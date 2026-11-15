// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::super::clip_candidates::{AliasKind, AliasProofs, Fact, Site, Target};
use super::super::origins;
use super::super::projection::DefinitionProjection;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{
    CandidateState, Identity, MediaError, MediaWriter, Reference, ReferenceResolution,
    ReferenceTarget, origin_error,
};

pub(super) fn observe(
    projection: &DefinitionProjection<'_, '_>,
    (name, indices): (&str, [u64; 2]),
    reference: &Reference,
    prepared: &PreparedRecordOrigins,
    selector: &OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let directory = projection.context.directory;
    if projection.repaired_strings || !directory.uses_definition_scope() {
        return Ok(());
    }
    let name_proof = prepared
        .visit("/data/payload/name")
        .ok_or_else(origin_error::foreign)?;
    let site_proof = prepared
        .visit("/data/payload/authored_path")
        .ok_or_else(origin_error::foreign)?;
    let observe = |ordinal: usize, target: &ReferenceTarget| {
        let Identity::Entry { entry, .. } = &target.source else {
            return Err(origin_error::foreign());
        };
        let archive = directory.index.archive;
        let physical = archive
            .entries()
            .get(entry.entry_index)
            .ok_or_else(origin_error::foreign)?;
        let target_proof = origins::physical_path(directory, physical, writer)?;
        let mut proof = writer.capture_origins();
        proof.include_original(selector)?;
        proof.include_original(&target_proof)?;
        directory.index.animation_clip_candidates.observe(
            Fact::Alias {
                site: Site {
                    entry: projection.context.entry.index(),
                    indices,
                },
                ordinal: ordinal as u64,
                kind: AliasKind::NamedCaf,
                name,
                target: Target::Caf {
                    entry: entry.entry_index,
                },
                origins: AliasProofs {
                    occurrence: &proof.finish(),
                    name: name_proof,
                    site: site_proof,
                    target: &target_proof,
                    target_clip: None,
                },
            },
            writer,
        )
    };
    visit_selected(reference.resolution(), observe)
}

fn visit_selected(
    resolution: &ReferenceResolution,
    mut observe: impl FnMut(usize, &ReferenceTarget) -> Result<(), MediaError>,
) -> Result<(), MediaError> {
    match resolution {
        ReferenceResolution::Resolved { target } => observe(0, target),
        ReferenceResolution::Ambiguous { candidates }
        | ReferenceResolution::Unavailable {
            known_candidates: candidates,
            ..
        } => {
            for (ordinal, candidate) in candidates.iter().enumerate() {
                if matches!(candidate.state, CandidateState::Selected) {
                    observe(ordinal, &candidate.target)?;
                }
            }
            Ok(())
        }
        ReferenceResolution::Missing | ReferenceResolution::NotSelected { .. } => Ok(()),
    }
}
