// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_audio::{Codec, LinearWavWrite};

use crate::sound::{Sound, WemFormatField, WemOriginField, WemOriginPlan};
use crate::{
    DecimalU64, FieldOrigin, InputScope, OriginArtifactBinding, OriginOutputAddress,
    OriginWavField, SourceInventory, SourceLocation, SourceScope, SourceUse, SourceUseRole,
    SourceUses, write_origin_companion,
};

pub(super) struct WavOrigins {
    inventory: SourceInventory,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
}

impl WavOrigins {
    pub(super) fn capture(sound: &Sound, written: LinearWavWrite) -> io::Result<Self> {
        let uses = SourceUses::new();
        let payload = sound
            .source_input()
            .map(|input| uses.observe(input, InputScope::Payload))
            .transpose()
            .map_err(io::Error::other)?;
        let wem = sound.wem();
        let plan = WemOriginPlan::new(wem.info_source(), wem.codec(), wem.sample_count());
        let mut evidence = WavEvidence {
            fields: Vec::new(),
            uses: &uses,
            payload: payload.as_ref(),
            plan,
        };
        evidence.header(sound, written)?;
        evidence.signal(sound, written)?;
        Ok(Self {
            inventory: uses.freeze().map_err(io::Error::other)?,
            fields: evidence.fields,
        })
    }

    pub(super) fn write(
        &self,
        writer: &mut dyn Write,
        binding: &OriginArtifactBinding,
    ) -> io::Result<()> {
        write_origin_companion(
            writer,
            binding,
            &self.inventory,
            self.fields
                .iter()
                .map(|(address, origin)| (address, origin)),
        )
    }
}

struct WavEvidence<'uses, 'info> {
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
    uses: &'uses SourceUses,
    payload: Option<&'uses SourceScope>,
    plan: WemOriginPlan<'info>,
}

impl WavEvidence<'_, '_> {
    fn header(&mut self, sound: &Sound, written: LinearWavWrite) -> io::Result<()> {
        const PCM_FORMAT_END: u64 = 36;
        const EXTENSIBLE_FORMAT_END: u64 = 60;

        use OriginWavField::{
            BitsPerSample, BlockAlign, ByteRate, ChannelMask, Channels, FormatLength, FormatTag,
            RiffLength, SampleRate,
        };

        let channels = self.planned(WemOriginField::Channels)?;
        let rate = self.planned(WemOriginField::SampleRate)?;
        let mut shape = self.shape(sound, written)?;
        shape.extend(self.planned(WemOriginField::Loop)?);
        if sound.wem().loop_points().is_some() {
            shape.extend(self.planned(WemOriginField::LoopStart)?);
            shape.extend(self.planned(WemOriginField::LoopEnd)?);
        }

        self.field(RiffLength, 4, 8, &shape);
        self.field(FormatLength, 16, 20, &channels);
        self.field(FormatTag, 20, 22, &channels);
        self.field(Channels, 22, 24, &channels);
        self.field(SampleRate, 24, 28, &rate);
        let byte_rate: Vec<_> = channels.iter().chain(&rate).cloned().collect();
        self.field(ByteRate, 28, 32, &byte_rate);
        self.field(BlockAlign, 32, 34, &channels);
        self.insert(BitsPerSample, 34, 36, &[], true);

        let loop_base = if written.report.channels > 2 {
            let mut mask = self.planned(WemOriginField::Format(WemFormatField::ChannelMask {
                layout_present: sound.wem().metadata().channel_layout().is_some(),
            }))?;
            mask.extend_from_slice(&channels);
            self.field(ChannelMask, 40, 44, &mask);
            EXTENSIBLE_FORMAT_END
        } else {
            PCM_FORMAT_END
        };
        if written.report.loop_points.is_some() {
            self.loops(loop_base, &shape, &rate)?;
        }
        Ok(())
    }

    fn loops(&mut self, base: u64, shape: &[SourceUse], rate: &[SourceUse]) -> io::Result<()> {
        use OriginWavField::{LoopCount, LoopEndInclusive, LoopStart, SamplePeriod};

        let mut sources = shape.to_vec();
        sources.extend_from_slice(rate);
        self.field(SamplePeriod, base + 16, base + 20, &sources);
        self.field(LoopCount, base + 36, base + 40, shape);
        let mut start = self.planned(WemOriginField::LoopStart)?;
        start.extend_from_slice(shape);
        self.field(LoopStart, base + 52, base + 56, &start);
        let mut end = self.planned(WemOriginField::LoopEnd)?;
        end.extend_from_slice(shape);
        self.field(LoopEndInclusive, base + 56, base + 60, &end);
        Ok(())
    }

    fn signal(&mut self, sound: &Sound, written: LinearWavWrite) -> io::Result<()> {
        use OriginWavField::{DataLength, PcmSamples};

        let mut shape = self.shape(sound, written)?;
        let offset = written.output_audio_start;
        let length_at = offset
            .checked_sub(4)
            .ok_or_else(|| io::Error::other("WAV has no data length word"))?;
        self.field(DataLength, length_at, offset, &shape);
        if let Some(payload) = self.payload
            && (written.source_audio_bytes != 0 || sound.wem().data().is_empty())
        {
            let start = sound.wem().info_source().data_body_offset();
            let end = start
                .checked_add(written.source_audio_bytes)
                .ok_or_else(|| io::Error::other("WEM sample extent overflows"))?;
            if let Some(stride) = written.ima_sub_block_bytes {
                shape.extend(ima_source(payload, start, written, stride)?);
            } else {
                let namespace = if sound.wem().codec() == Codec::Pcm {
                    "wemPcmFrames"
                } else {
                    "wemAdpcmBlocks"
                };
                shape.push(
                    payload
                        .at(
                            SourceLocation::Subresource {
                                namespace: namespace.to_owned(),
                                indices: vec![start, end],
                                field: "decodedAudio".into(),
                            },
                            SourceUseRole::Conversion,
                        )
                        .map_err(io::Error::other)?,
                );
            }
        }
        self.field(PcmSamples, offset, written.report.size, &shape);
        Ok(())
    }

    fn shape(&self, sound: &Sound, written: LinearWavWrite) -> io::Result<Vec<SourceUse>> {
        let mut uses = self.planned(WemOriginField::Channels)?;
        uses.extend(self.planned(WemOriginField::Codec)?);
        if sound.wem().codec() != Codec::Pcm {
            uses.extend(self.planned(WemOriginField::Format(WemFormatField::BlockAlign))?);
        }
        if let Some(payload) = self.payload {
            let offset = sound.wem().info_source().data_body_offset();
            uses.push(
                payload
                    .at(
                        SourceLocation::Subresource {
                            namespace: "wemChunk".into(),
                            indices: vec![offset],
                            field: "bodyLength".into(),
                        },
                        SourceUseRole::Conversion,
                    )
                    .map_err(io::Error::other)?,
            );
            if written.source_audio_bytes > sound.wem().data().len() as u64 {
                return Err(io::Error::other(
                    "WAV used bytes outside the selected WEM DATA chunk",
                ));
            }
        }
        Ok(uses)
    }

    fn planned(&self, field: WemOriginField) -> io::Result<Vec<SourceUse>> {
        let Some(payload) = self.payload else {
            return Ok(Vec::new());
        };
        self.plan
            .field(field)
            .map_err(io::Error::other)?
            .iter()
            .map(|site| site.bind(payload).map_err(io::Error::other))
            .collect()
    }

    fn field(&mut self, field: OriginWavField, start: u64, end: u64, sources: &[SourceUse]) {
        self.insert(field, start, end, sources, self.payload.is_some());
    }

    fn insert(
        &mut self,
        field: OriginWavField,
        start: u64,
        end: u64,
        sources: &[SourceUse],
        complete: bool,
    ) {
        let address = OriginOutputAddress::WavBytes {
            start: DecimalU64::new(start),
            end_exclusive: DecimalU64::new(end),
            field,
        };
        self.fields
            .push((address, self.uses.origin(sources, complete)));
    }
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
