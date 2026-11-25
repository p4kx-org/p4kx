// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_audio::Codec;

use crate::sound::origins::{SoundEvidence, SoundOrigins, insert_summary};
use crate::sound::{Sound, WemFormatField, WemOriginField};
use crate::{
    DecimalU64, FieldOrigin, FieldOrigins, OriginOutputAddress, OriginWavField, SourceLocation,
    SourceUse, SourceUseRole, SourceUses,
};

use super::written::WavWrite;

#[path = "sound_wav_sources.rs"]
mod sources;

pub(super) fn capture(sound: &Sound, written: &WavWrite) -> io::Result<SoundOrigins> {
    let uses = SourceUses::new();
    let mut capture = WavEvidence {
        evidence: SoundEvidence::observe(sound, &uses)?,
        fields: Vec::new(),
    };
    capture.header(sound, written)?;
    capture.signal(sound, written)?;
    let summary = capture.summary(sound, written)?;
    SoundOrigins::new(&uses, capture.fields, summary)
}

struct WavEvidence<'uses, 'sound> {
    evidence: SoundEvidence<'uses, 'sound>,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
}

impl WavEvidence<'_, '_> {
    fn header(&mut self, sound: &Sound, written: &WavWrite) -> io::Result<()> {
        const PCM_FORMAT_END: u64 = 36;
        const EXTENSIBLE_FORMAT_END: u64 = 60;

        use OriginWavField::{
            BitsPerSample, BlockAlign, ByteRate, ChannelMask, Channels, FormatLength, FormatTag,
            RiffLength, SampleRate,
        };

        let channels = self.evidence.planned(WemOriginField::Channels)?;
        let rate = self.evidence.planned(WemOriginField::SampleRate)?;
        let mut shape = self.shape(sound, written)?;
        shape.extend(self.evidence.planned(WemOriginField::Loop)?);
        if sound.wem().loop_points().is_some() {
            shape.extend(self.evidence.planned(WemOriginField::LoopStart)?);
            shape.extend(self.evidence.planned(WemOriginField::LoopEnd)?);
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

        let loop_base = if written.report().channels > 2 {
            let mut mask =
                self.evidence
                    .planned(WemOriginField::Format(WemFormatField::ChannelMask {
                        layout_present: sound.wem().metadata().channel_layout().is_some(),
                    }))?;
            mask.extend_from_slice(&channels);
            self.field(ChannelMask, 40, 44, &mask);
            EXTENSIBLE_FORMAT_END
        } else {
            PCM_FORMAT_END
        };
        if written.report().loop_points.is_some() {
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
        let mut start = self.evidence.planned(WemOriginField::LoopStart)?;
        start.extend_from_slice(shape);
        self.field(LoopStart, base + 52, base + 56, &start);
        let mut end = self.evidence.planned(WemOriginField::LoopEnd)?;
        end.extend_from_slice(shape);
        self.field(LoopEndInclusive, base + 56, base + 60, &end);
        Ok(())
    }

    fn signal(&mut self, sound: &Sound, written: &WavWrite) -> io::Result<()> {
        use OriginWavField::{DataLength, PcmSamples};

        let mut shape = self.shape(sound, written)?;
        let offset = written.output_audio_start();
        let length_at = offset
            .checked_sub(4)
            .ok_or_else(|| io::Error::other("WAV has no data length word"))?;
        self.field(DataLength, length_at, offset, &shape);
        if let Some(payload) = self.evidence.payload.as_ref() {
            sources::samples(payload, sound, written, &mut shape)?;
        }
        self.field(PcmSamples, offset, written.report().size, &shape);
        Ok(())
    }

    fn shape(&self, sound: &Sound, written: &WavWrite) -> io::Result<Vec<SourceUse>> {
        let mut uses = self.evidence.planned(WemOriginField::Channels)?;
        uses.extend(self.evidence.planned(WemOriginField::Codec)?);
        if matches!(written, WavWrite::Linear(_)) && sound.wem().codec() != Codec::Pcm {
            uses.extend(
                self.evidence
                    .planned(WemOriginField::Format(WemFormatField::BlockAlign))?,
            );
        }
        if let Some(payload) = self.evidence.payload.as_ref() {
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
            sources::shape(payload, sound, written, &mut uses)?;
        }
        Ok(uses)
    }

    fn summary(&self, sound: &Sound, written: &WavWrite) -> io::Result<FieldOrigins> {
        use OriginWavField::{Channels, DataLength, RiffLength, SampleRate};

        let mut fields = FieldOrigins::new();
        for (name, field) in [
            ("/channels", Channels),
            ("/sample_rate", SampleRate),
            ("/sample_count", DataLength),
            ("/size", RiffLength),
        ] {
            let origin = self
                .fields
                .iter()
                .find_map(|(address, origin)| match address {
                    OriginOutputAddress::WavBytes { field: actual, .. } if *actual == field => {
                        Some(origin)
                    }
                    _ => None,
                })
                .ok_or_else(|| io::Error::other(format!("missing WAV field for {name}")))?;
            insert_summary(&mut fields, name, origin.clone())?;
        }
        insert_summary(&mut fields, "/sound", self.evidence.name_origin()?)?;
        self.summary_loop(sound, written, &mut fields)?;
        Ok(fields)
    }

    fn summary_loop(
        &self,
        sound: &Sound,
        written: &WavWrite,
        fields: &mut FieldOrigins,
    ) -> io::Result<()> {
        let mut selection = self.evidence.planned(WemOriginField::Loop)?;
        if sound.wem().loop_points().is_some() {
            selection.extend(self.shape(sound, written)?);
            selection.extend(self.evidence.planned(WemOriginField::LoopStart)?);
            selection.extend(self.evidence.planned(WemOriginField::LoopEnd)?);
        }
        let complete = self.evidence.payload.as_ref().is_some();
        let origin = self.evidence.uses.origin(&selection, complete);
        if written.report().loop_points.is_some() {
            for (path, field) in [
                ("/loop/start", WemOriginField::LoopStart),
                ("/loop/end", WemOriginField::LoopEnd),
                ("/loop/frames", WemOriginField::LoopFrames),
            ] {
                let mut inputs = self.evidence.planned(field)?;
                inputs.extend(selection.iter().cloned());
                insert_summary(fields, path, self.evidence.uses.origin(&inputs, complete))?;
            }
        }
        insert_summary(fields, "/loop", origin)
    }

    fn field(&mut self, field: OriginWavField, start: u64, end: u64, sources: &[SourceUse]) {
        self.insert(
            field,
            start,
            end,
            sources,
            self.evidence.payload.as_ref().is_some(),
        );
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
            .push((address, self.evidence.uses.origin(sources, complete)));
    }
}
