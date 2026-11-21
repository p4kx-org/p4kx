// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;

use crate::audio_control_payload::AtlPosition;
use crate::audio_control_source::AudioControlSource;
use crate::audio_record_origins as records;
use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(crate) fn observation(
    input: &MediaOriginInput,
    kind: AudioControlSource,
    position: AtlPosition,
    field: &str,
    role: SourceUseRole,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let evidence = with_source_site(kind, position, field, |site| {
        records::capture(input, [(site, role)], writer)
    })?;
    prepared(input, evidence, writer)
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
    with_site(position, field, |site| {
        records::capture(input, [(site, role)], writer)
    })
}

pub(crate) fn with_site<T>(
    position: AtlPosition,
    field: &str,
    visit: impl FnOnce(OriginSite<'_>) -> T,
) -> T {
    with_source_site(AudioControlSource::Atl, position, field, visit)
}

fn with_source_site<T>(
    kind: AudioControlSource,
    position: AtlPosition,
    field: &str,
    visit: impl FnOnce(OriginSite<'_>) -> T,
) -> T {
    let namespaces = match kind {
        AudioControlSource::Atl => SyntaxNamespaces {
            xml: "atl-xml",
            node: "atl-cryxml-node",
            attribute: "atl-cryxml-attribute",
        },
        AudioControlSource::CigAudio => SyntaxNamespaces {
            xml: "cig-audio-xml",
            node: "cig-audio-cryxml-node",
            attribute: "cig-audio-cryxml-attribute",
        },
    };
    match position {
        AtlPosition::Text {
            start,
            end_exclusive,
        } => visit(OriginSite::new(
            namespaces.xml,
            &[start.get(), end_exclusive.get()],
            field,
        )),
        AtlPosition::CryXmlNode { node_index } | AtlPosition::CryXmlContent { node_index } => {
            visit(OriginSite::new(
                namespaces.node,
                &[u64::from(node_index)],
                field,
            ))
        }
        AtlPosition::CryXmlAttribute {
            node_index,
            attribute_index,
        } => visit(OriginSite::new(
            namespaces.attribute,
            &[u64::from(node_index), u64::from(attribute_index)],
            field,
        )),
    }
}

struct SyntaxNamespaces {
    xml: &'static str,
    node: &'static str,
    attribute: &'static str,
}

pub(crate) fn document(
    input: &MediaOriginInput,
    kind: AudioControlSource,
    size: u64,
    role: SourceUseRole,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = records::prepare_for(input, OriginProfile::AudioControl)?;
    let evidence = file(input, kind, size, role, writer)?;
    records::subtree(&mut prepared, "/data/payload", evidence, writer)?;
    Ok(prepared)
}

pub(crate) fn file(
    input: &MediaOriginInput,
    kind: AudioControlSource,
    size: u64,
    role: SourceUseRole,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let namespace = match kind {
        AudioControlSource::Atl => "atl-definition-file",
        AudioControlSource::CigAudio => "cig-audio-graph-file",
    };
    records::capture(
        input,
        [(OriginSite::new(namespace, &[0, size], "document"), role)],
        writer,
    )
}
