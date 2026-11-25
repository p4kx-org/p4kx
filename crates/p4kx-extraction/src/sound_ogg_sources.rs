// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;

use p4kx_audio::{WemOpusOggEvidence, WemVorbisOggEvidence};

use crate::sound::WemOriginField;
use crate::sound::origins::SoundEvidence;
use crate::{FieldOrigin, Sound, SourceLocation, SourceUse, SourceUseRole};

use super::OggWrite;

const OPUS: &str = "wemOpus";
const VORBIS: &str = "wemVorbis";

pub(super) struct OggSources {
    pub(super) codec: FieldOrigin,
    pub(super) channels: FieldOrigin,
    pub(super) sample_rate: FieldOrigin,
    pub(super) sample_count: FieldOrigin,
    pub(super) codebooks: FieldOrigin,
    pub(super) identification: FieldOrigin,
    pub(super) comments_and_setup: FieldOrigin,
    pub(super) audio: FieldOrigin,
    pub(super) embedded_comments: Option<FieldOrigin>,
}

impl OggSources {
    pub(super) fn capture(
        sound: &Sound,
        written: &OggWrite,
        evidence: &SoundEvidence<'_, '_>,
    ) -> io::Result<Self> {
        let codec = planned(evidence, WemOriginField::Codec)?;
        let channels = planned(evidence, WemOriginField::Channels)?;
        let sample_rate = planned(evidence, WemOriginField::SampleRate)?;
        let mut length = Vec::new();
        if let Some(payload) = &evidence.payload {
            length.push(
                payload
                    .at(
                        SourceLocation::Subresource {
                            namespace: "wemChunk".to_owned(),
                            indices: vec![sound.wem().info_source().data_body_offset()],
                            field: "bodyLength".to_owned(),
                        },
                        SourceUseRole::Selector,
                    )
                    .map_err(io::Error::other)?,
            );
        }
        let length = evidence.origin(&length);
        let context = Context {
            evidence,
            codec: &codec,
            channels: &channels,
            sample_rate: &sample_rate,
            length: &length,
        };
        let groups = match written {
            OggWrite::Opus(written) => opus(written, &context)?,
            OggWrite::Vorbis(written) => vorbis(written, &context)?,
        };
        let channels = if let Some(embedded) = &groups.embedded_channels {
            evidence.combine([&channels, embedded])?
        } else {
            channels
        };
        Ok(Self {
            codec,
            channels,
            sample_rate,
            sample_count: groups.sample_count,
            codebooks: groups.codebooks,
            identification: groups.identification,
            comments_and_setup: groups.comments_and_setup,
            audio: groups.audio,
            embedded_comments: groups.embedded_comments,
        })
    }
}

struct Context<'capture, 'uses, 'sound> {
    evidence: &'capture SoundEvidence<'uses, 'sound>,
    codec: &'capture FieldOrigin,
    channels: &'capture FieldOrigin,
    sample_rate: &'capture FieldOrigin,
    length: &'capture FieldOrigin,
}

struct Groups {
    identification: FieldOrigin,
    comments_and_setup: FieldOrigin,
    audio: FieldOrigin,
    sample_count: FieldOrigin,
    codebooks: FieldOrigin,
    embedded_channels: Option<FieldOrigin>,
    embedded_comments: Option<FieldOrigin>,
}

fn opus(written: &WemOpusOggEvidence, context: &Context<'_, '_, '_>) -> io::Result<Groups> {
    use SourceUseRole::{Conversion, Selector};

    let evidence = context.evidence;
    let ranges = &written.ranges;
    let frame_format = single(evidence, OPUS, "format", &ranges.format, Conversion)?;
    let embedded = written.embedded_header_pages.as_ref();
    let original_head = embedded
        .map(|header| {
            single(
                evidence,
                OPUS,
                "embeddedIdentificationPages",
                &header.identification,
                Conversion,
            )
        })
        .transpose()?;
    let tags = embedded
        .map(|header| {
            single(
                evidence,
                OPUS,
                "embeddedCommentPages",
                &header.comments,
                Conversion,
            )
        })
        .transpose()?;
    let head = opus_head(written, context, &frame_format, original_head.as_ref())?;
    let mut checked = Vec::new();
    let checked_audio = written.replayed_audio_packets.start..ranges.checked_data.end;
    evidence.append_range(&mut checked, OPUS, "audioChecked", &checked_audio, Selector)?;
    if let Some(table) = &ranges.seek_table {
        evidence.append_range(&mut checked, OPUS, "seekTableChecked", table, Selector)?;
    }
    let checked = evidence.origin(&checked);
    let count = evidence.combine([context.length, &head, &frame_format, &checked])?;
    let mut replay = Vec::new();
    evidence.append_range(
        &mut replay,
        OPUS,
        "audioReplayed",
        &written.replayed_audio_packets,
        Conversion,
    )?;
    if let Some(table) = &written.replayed_seek_table {
        evidence.append_range(&mut replay, OPUS, "seekTableReplayed", table, Selector)?;
    }
    let replay = evidence.origin(&replay);
    Ok(Groups {
        identification: head,
        comments_and_setup: evidence.combine(std::iter::once(context.codec).chain(&tags))?,
        audio: evidence.combine([&count, &replay])?,
        sample_count: count,
        codebooks: context.codec.clone(),
        embedded_channels: original_head,
        embedded_comments: tags,
    })
}

fn opus_head(
    written: &WemOpusOggEvidence,
    context: &Context<'_, '_, '_>,
    format: &FieldOrigin,
    original: Option<&FieldOrigin>,
) -> io::Result<FieldOrigin> {
    let evidence = context.evidence;
    if let Some(original) = original {
        return evidence.combine([context.codec, original]);
    }
    let mut preskip = Vec::new();
    if let Some(source) = &written.switch_pre_skip {
        evidence.append_range(
            &mut preskip,
            OPUS,
            "switchFirstPacketSize",
            &source.packet_size,
            SourceUseRole::Selector,
        )?;
        evidence.append_range(
            &mut preskip,
            OPUS,
            "switchFirstDurationHeader",
            &source.duration_header,
            SourceUseRole::Conversion,
        )?;
    }
    let preskip = evidence.origin(&preskip);
    evidence.combine([context.codec, context.channels, format, &preskip])
}

fn vorbis(written: &WemVorbisOggEvidence, context: &Context<'_, '_, '_>) -> io::Result<Groups> {
    use SourceUseRole::{Conversion, Selector};

    let evidence = context.evidence;
    let ranges = &written.ranges;
    let configuration = single(
        evidence,
        VORBIS,
        "configuration",
        &ranges.header,
        Conversion,
    )?;
    let rate = single(
        evidence,
        VORBIS,
        "averageByteRate",
        &ranges.average_byte_rate,
        Conversion,
    )?;
    let identification = evidence.combine([
        context.codec,
        context.channels,
        context.sample_rate,
        &rate,
        &configuration,
    ])?;

    let setup = single(
        evidence,
        VORBIS,
        "setupPacket",
        &ranges.setup_packet,
        Conversion,
    )?;
    let selection = single(
        evidence,
        VORBIS,
        "codebookAudio",
        &written.codebook_audio_packets,
        Selector,
    )?;
    let codebooks = evidence.combine([&identification, &setup, &selection])?;
    let replay = single(
        evidence,
        VORBIS,
        "audioReplayed",
        &written.replayed_audio_packets,
        Conversion,
    )?;
    let count = evidence.combine([&codebooks, context.length, &replay])?;
    Ok(Groups {
        identification,
        comments_and_setup: codebooks.clone(),
        audio: count.clone(),
        sample_count: count,
        codebooks,
        embedded_channels: None,
        embedded_comments: None,
    })
}

fn planned(evidence: &SoundEvidence<'_, '_>, field: WemOriginField) -> io::Result<FieldOrigin> {
    Ok(evidence.origin(&evidence.planned(field)?))
}

fn single(
    evidence: &SoundEvidence<'_, '_>,
    namespace: &str,
    field: &str,
    bytes: &Range<u64>,
    role: SourceUseRole,
) -> io::Result<FieldOrigin> {
    let mut uses: Vec<SourceUse> = Vec::new();
    evidence.append_range(&mut uses, namespace, field, bytes, role)?;
    Ok(evidence.origin(&uses))
}
