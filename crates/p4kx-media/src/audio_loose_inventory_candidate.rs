// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{InputScope, SourceUseRole};

use crate::audio_record_origins as origins;
use crate::directory_lookup::LooseWemCandidate;
use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, Status, Subresource, source};

use super::CANDIDATE_NAMESPACE;
use super::payload::{Candidate, LooseInventoryPayload, PayloadAccess, Placement};

pub(super) fn project(
    directory: &MediaOriginInput,
    candidate: LooseWemCandidate<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let original = writer.origins.input(candidate.archive, candidate.entry)?;
    let identity = source::identity(candidate.archive, candidate.entry);
    let placement = if identity.raw_path_hex.is_some() {
        Placement::Unavailable
    } else {
        candidate
            .name
            .language
            .map_or(Placement::Neutral, |folder| Placement::Language { folder })
    };
    let index = u64::try_from(candidate.entry.index())
        .map_err(|_| MediaError::Contract("loose WEM Entry index exceeds u64".into()))?;
    let mut prepared = origins::prepare_for(directory, OriginProfile::AudioWem)?;
    capture_origins(&original, &mut prepared, writer)?;
    prepared.write(
        writer,
        &Subresource::new(CANDIDATE_NAMESPACE, vec![index]),
        Status::Complete,
        &LooseInventoryPayload::LooseWemCandidate(Candidate {
            entry: identity,
            media_id: candidate.name.id,
            placement,
            selection: candidate.selection,
            payload_access: if candidate.key_required {
                PayloadAccess::KeyRequired
            } else {
                PayloadAccess::NotInspected
            },
        }),
        &[],
    )
}

fn capture_origins(
    input: &MediaOriginInput,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let path = directory_field(input, "path", SourceUseRole::Conversion, writer)?;
    origins::field(prepared, "/data/payload/media_id", path.clone(), writer)?;
    origins::subtree(prepared, "/data/payload/placement", path.clone(), writer)?;
    origins::subtree(prepared, "/data/payload/selection", path, writer)?;
    let access = directory_field(input, "encryption", SourceUseRole::Selector, writer)?;
    origins::field(prepared, "/data/payload/payload_access", access, writer)?;
    for field in ["entry_index", "path", "raw_path_hex"] {
        let original = directory_field(input, field, SourceUseRole::Payload, writer)?;
        origins::field(
            prepared,
            &format!("/data/payload/entry/{field}"),
            original,
            writer,
        )?;
    }
    let index = directory_field(input, "entry_index", SourceUseRole::Conversion, writer)?;
    origins::indices(prepared, index, writer)
}

fn directory_field(
    input: &MediaOriginInput,
    field: &str,
    role: SourceUseRole,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    capture.add(
        input,
        InputScope::ArchiveDirectoryMetadata,
        OriginSite::new("archive-directory", &[], field),
        role,
        writer,
    )?;
    Ok(capture.finish())
}
