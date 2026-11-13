// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{InputScope, SourceUseRole};

use crate::audio_control_payload::AtlPosition;
use crate::audio_record_origins as records;
use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(super) fn record(
    input: &MediaOriginInput,
    evidence: OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = records::prepare_for(input, OriginProfile::AnimationDefinition)?;
    records::indices(&mut prepared, evidence.clone(), writer)?;
    records::subtree(&mut prepared, "/data/payload", evidence, writer)?;
    Ok(prepared)
}

pub(super) fn position(
    input: &MediaOriginInput,
    position: AtlPosition,
    role: SourceUseRole,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let (namespace, coordinates, len) = match position {
        AtlPosition::Text {
            start,
            end_exclusive,
        } => (
            "animation-definition-xml",
            [start.get(), end_exclusive.get()],
            2,
        ),
        AtlPosition::CryXmlNode { node_index } | AtlPosition::CryXmlContent { node_index } => (
            "animation-definition-cryxml-node",
            [u64::from(node_index), 0],
            1,
        ),
        AtlPosition::CryXmlAttribute {
            node_index,
            attribute_index,
        } => (
            "animation-definition-cryxml-attribute",
            [u64::from(node_index), u64::from(attribute_index)],
            2,
        ),
    };
    let coordinates = coordinates
        .get(..len)
        .ok_or_else(crate::origin_error::counter)?;
    records::capture(
        input,
        [(OriginSite::new(namespace, coordinates, "declaration"), role)],
        writer,
    )
}

pub(super) fn document(
    input: &MediaOriginInput,
    entry: usize,
    size: u64,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let proof = records::capture(
        input,
        [(
            OriginSite::new("animation-definition-file", &[0, size], "document"),
            SourceUseRole::Payload,
        )],
        writer,
    )?;
    let mut prepared = records::prepare_for(input, OriginProfile::AnimationDefinition)?;
    records::subtree(&mut prepared, "/data/payload", proof.clone(), writer)?;
    records::subtree(&mut prepared, "/issues", proof, writer)?;
    let mut family = writer.capture_origins();
    family.add(
        input,
        InputScope::ArchiveDirectoryMetadata,
        OriginSite::new("archive-entry", &[entry as u64], "path"),
        SourceUseRole::Selector,
        writer,
    )?;
    records::field(
        &mut prepared,
        "/data/payload/family",
        family.finish(),
        writer,
    )?;
    Ok(prepared)
}
