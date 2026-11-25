// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;

use p4kx_audio::Wem;

use crate::sound::origins::{SoundEvidence, SoundOrigins, insert_summary};
use crate::sound::{SoundOriginInput, WemOriginField};
use crate::{
    DecimalU64, FieldOrigin, FieldOrigins, OggReport, OriginOggField, OriginOutputAddress,
    SourceUses,
};

use super::written::OggWrite;

#[path = "sound_ogg_sources.rs"]
mod sources;
use sources::OggSources;

pub(super) fn capture(
    wem: &Wem<impl AsRef<[u8]>>,
    input: Option<SoundOriginInput<'_>>,
    written: &OggWrite,
) -> io::Result<SoundOrigins> {
    let uses = SourceUses::new();
    let evidence = SoundEvidence::observe(wem, input, &uses)?;
    let sources = OggSources::capture(wem, written, &evidence)?;
    let loops = LoopOrigins::capture(wem, written.report(), &evidence, &sources)?;
    let comments = evidence.combine([&sources.comments_and_setup, &loops.selection])?;
    let size = evidence.combine([&sources.identification, &comments, &sources.audio])?;

    let output = written.output();
    let fields = vec![
        group(
            OriginOggField::Identification,
            &output.identification,
            sources.identification,
        ),
        group(
            OriginOggField::CommentsAndSetup,
            &output.comments_and_setup,
            comments,
        ),
        group(OriginOggField::Audio, &output.audio, sources.audio),
    ];

    let duration = evidence.combine([&sources.sample_count, &sources.sample_rate])?;
    let mut summary = FieldOrigins::new();
    for (path, origin) in [
        ("/sound", evidence.name_origin()?),
        ("/codec", sources.codec),
        ("/channels", sources.channels),
        ("/sample_rate", sources.sample_rate),
        ("/sample_count", sources.sample_count),
        ("/duration_seconds", duration),
        ("/codebooks", sources.codebooks),
        ("/size", size),
    ] {
        insert_summary(&mut summary, path, origin)?;
    }
    loops.insert(&mut summary)?;
    SoundOrigins::new(&uses, fields, summary)
}

const fn group(
    field: OriginOggField,
    output: &Range<u64>,
    origin: FieldOrigin,
) -> (OriginOutputAddress, FieldOrigin) {
    (
        OriginOutputAddress::OggBytes {
            start: DecimalU64::new(output.start),
            end_exclusive: DecimalU64::new(output.end),
            field,
        },
        origin,
    )
}

struct LoopOrigins {
    selection: FieldOrigin,
    points: Option<LoopPoints>,
}

struct LoopPoints {
    start: FieldOrigin,
    end: FieldOrigin,
    frames: FieldOrigin,
}

impl LoopOrigins {
    fn capture(
        wem: &Wem<impl AsRef<[u8]>>,
        report: OggReport,
        evidence: &SoundEvidence<'_, '_>,
        sources: &OggSources,
    ) -> io::Result<Self> {
        let mut inputs = evidence.planned(WemOriginField::Loop)?;
        if wem.loop_points().is_some() {
            inputs.extend(evidence.planned(WemOriginField::LoopStart)?);
            inputs.extend(evidence.planned(WemOriginField::LoopEnd)?);
        }
        let declared = evidence.origin(&inputs)?;
        let selection = if wem.loop_points().is_some() {
            evidence.combine(
                [&declared, &sources.sample_count]
                    .into_iter()
                    .chain(&sources.embedded_comments),
            )?
        } else {
            declared
        };
        let points = if report.loop_points.is_some() {
            let origin = |field| {
                let value = evidence.origin(&evidence.planned(field)?)?;
                evidence.combine([&value, &selection])
            };
            Some(LoopPoints {
                start: origin(WemOriginField::LoopStart)?,
                end: origin(WemOriginField::LoopEnd)?,
                frames: origin(WemOriginField::LoopFrames)?,
            })
        } else {
            None
        };
        Ok(Self { selection, points })
    }

    fn insert(self, summary: &mut FieldOrigins) -> io::Result<()> {
        insert_summary(summary, "/loop", self.selection)?;
        if let Some(points) = self.points {
            insert_summary(summary, "/loop/start", points.start)?;
            insert_summary(summary, "/loop/end", points.end)?;
            insert_summary(summary, "/loop/frames", points.frames)?;
        }
        Ok(())
    }
}
