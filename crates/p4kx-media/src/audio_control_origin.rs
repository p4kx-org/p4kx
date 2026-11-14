// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;

use crate::audio_control_payload::AtlPosition;
use crate::audio_record_origins as records;
use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(crate) fn observation(
    input: &MediaOriginInput,
    position: AtlPosition,
    field: &str,
    role: SourceUseRole,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    prepared(
        input,
        capture(input, position, field, role, writer)?,
        writer,
    )
}

pub(crate) fn prepared(
    input: &MediaOriginInput,
    evidence: OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = records::prepare_for(input, OriginProfile::AudioControl)?;
    records::indices(&mut prepared, evidence.clone(), writer)?;
    records::subtree(&mut prepared, "/data/payload", evidence, writer)?;
    Ok(prepared)
}

pub(crate) fn capture(
    input: &MediaOriginInput,
    position: AtlPosition,
    field: &str,
    role: SourceUseRole,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    match position {
        AtlPosition::Text {
            start,
            end_exclusive,
        } => records::capture(
            input,
            [(
                OriginSite::new("atl-xml", &[start.get(), end_exclusive.get()], field),
                role,
            )],
            writer,
        ),
        AtlPosition::CryXmlNode { node_index } | AtlPosition::CryXmlContent { node_index } => {
            records::capture(
                input,
                [(
                    OriginSite::new("atl-cryxml-node", &[u64::from(node_index)], field),
                    role,
                )],
                writer,
            )
        }
        AtlPosition::CryXmlAttribute {
            node_index,
            attribute_index,
        } => records::capture(
            input,
            [(
                OriginSite::new(
                    "atl-cryxml-attribute",
                    &[u64::from(node_index), u64::from(attribute_index)],
                    field,
                ),
                role,
            )],
            writer,
        ),
    }
}

pub(crate) fn document(
    input: &MediaOriginInput,
    size: u64,
    role: SourceUseRole,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = records::prepare_for(input, OriginProfile::AudioControl)?;
    let evidence = file(input, size, role, writer)?;
    records::subtree(&mut prepared, "/data/payload", evidence, writer)?;
    Ok(prepared)
}

pub(crate) fn file(
    input: &MediaOriginInput,
    size: u64,
    role: SourceUseRole,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    records::capture(
        input,
        [(
            OriginSite::new("atl-definition-file", &[0, size], "document"),
            role,
        )],
        writer,
    )
}
