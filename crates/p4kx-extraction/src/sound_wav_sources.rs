// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;

use p4kx_audio::{Codec, LinearWavWrite, Wem};

use super::WavWrite;
use crate::sound::origins::range_use;
use crate::{SourceLocation, SourceScope, SourceUse, SourceUseRole};

pub(super) fn shape(
    payload: &SourceScope,
    wem: &Wem<impl AsRef<[u8]>>,
    written: &WavWrite,
    uses: &mut Vec<SourceUse>,
) -> io::Result<()> {
    let selector = SourceUseRole::Selector;
    match written {
        WavWrite::Linear(written) => {
            if written.source_audio_bytes > wem.data().len() as u64 {
                return Err(io::Error::other(
                    "WAV used bytes outside the selected WEM DATA chunk",
                ));
            }
        }
        WavWrite::Opus(written) => {
            let ranges = &written.ranges;
            for (field, range) in [
                ("format", Some(&ranges.format)),
                ("seekTableChecked", ranges.seek_table.as_ref()),
                ("embeddedHeaders", ranges.embedded_headers.as_ref()),
                ("audioChecked", Some(&ranges.checked_data)),
            ] {
                if let Some(range) = range {
                    append(payload, uses, "wemOpus", field, range, selector)?;
                }
            }
        }
        WavWrite::Vorbis(written) => {
            let ranges = &written.ranges;
            for (field, range) in [
                ("averageByteRate", &ranges.average_byte_rate),
                ("configuration", &ranges.header),
                ("setupPacket", &ranges.setup_packet),
                ("audioChecked", &ranges.audio_packets),
            ] {
                append(payload, uses, "wemVorbis", field, range, selector)?;
            }
        }
    }
    Ok(())
}

pub(super) fn samples(
    payload: &SourceScope,
    wem: &Wem<impl AsRef<[u8]>>,
    written: &WavWrite,
    uses: &mut Vec<SourceUse>,
) -> io::Result<()> {
    let conversion = SourceUseRole::Conversion;
    let selector = SourceUseRole::Selector;
    match written {
        WavWrite::Linear(written) => linear_samples(payload, wem, *written, uses),
        WavWrite::Opus(written) => {
            let ranges = &written.ranges;
            append(
                payload,
                uses,
                "wemOpus",
                "audioReplayed",
                &ranges.emitted_data,
                conversion,
            )?;
            if let Some(range) = &written.replayed_seek_table {
                append(
                    payload,
                    uses,
                    "wemOpus",
                    "seekTableReplayed",
                    range,
                    selector,
                )?;
            }
            Ok(())
        }
        WavWrite::Vorbis(written) => {
            let decoded = &written.decoded_audio_packets;
            if !decoded.is_empty() {
                append(
                    payload,
                    uses,
                    "wemVorbis",
                    "setupPacket",
                    &written.ranges.setup_packet,
                    conversion,
                )?;
            }
            append(
                payload,
                uses,
                "wemVorbis",
                "audioDecoded",
                decoded,
                conversion,
            )?;
            let lookahead = decoded.end..written.replayed_audio_packets.end;
            append(
                payload,
                uses,
                "wemVorbis",
                "audioLookahead",
                &lookahead,
                selector,
            )
        }
    }
}

fn append(
    payload: &SourceScope,
    uses: &mut Vec<SourceUse>,
    namespace: &str,
    field: &str,
    range: &Range<u64>,
    role: SourceUseRole,
) -> io::Result<()> {
    if !range.is_empty() {
        uses.push(range_use(payload, namespace, field, range, role)?);
    }
    Ok(())
}

fn linear_samples(
    payload: &SourceScope,
    wem: &Wem<impl AsRef<[u8]>>,
    written: LinearWavWrite,
    uses: &mut Vec<SourceUse>,
) -> io::Result<()> {
    if written.source_audio_bytes == 0 && !wem.data().is_empty() {
        return Ok(());
    }
    let start = wem.info_source().data_body_offset();
    let end = start
        .checked_add(written.source_audio_bytes)
        .ok_or_else(|| io::Error::other("WEM sample extent overflows"))?;
    if let Some(stride) = written.ima_sub_block_bytes {
        uses.extend(ima_source(payload, start, written, stride)?);
    } else {
        let namespace = if wem.codec() == Codec::Pcm {
            "wemPcmFrames"
        } else {
            "wemAdpcmBlocks"
        };
        uses.push(range_use(
            payload,
            namespace,
            "decodedAudio",
            &(start..end),
            SourceUseRole::Conversion,
        )?);
    }
    Ok(())
}

/// Binds byte runs for the history, index and nibbles of each used IMA sub-block.
fn ima_source(
    payload: &SourceScope,
    start: u64,
    written: LinearWavWrite,
    stride: u64,
) -> io::Result<Vec<SourceUse>> {
    const IMA_USED_HEADER_BYTES: u64 = 3;
    const IMA_HEADER_BYTES: u64 = 4;
    let channels = u64::from(written.report.channels);
    let block = channels * stride;
    let tail = written.source_audio_bytes % block;
    let whole = written.source_audio_bytes - tail;
    let complete = (start, whole / stride, stride);
    let partial = (
        start + whole,
        if tail > 0 { channels } else { 0 },
        tail / channels,
    );
    let mut sources = Vec::new();
    for (position, count, step) in [complete, partial] {
        if count == 0 {
            continue;
        }
        for (offset, length) in [
            (0, IMA_USED_HEADER_BYTES),
            (IMA_HEADER_BYTES, step - IMA_HEADER_BYTES),
        ] {
            sources.push(
                payload
                    .at(
                        SourceLocation::Subresource {
                            namespace: "wemImaAdpcmByteRuns".into(),
                            indices: vec![position, count, step, offset, length],
                            field: "decodedAudio".into(),
                        },
                        SourceUseRole::Conversion,
                    )
                    .map_err(io::Error::other)?,
            );
        }
    }
    Ok(sources)
}
