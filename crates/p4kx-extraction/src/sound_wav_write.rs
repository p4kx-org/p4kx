// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;
use std::path::Path;

use p4kx_audio::{Codec, LinearWavWrite, WavReport, WemOpusWavEvidence, WemVorbisWavEvidence};

use crate::sound::conversion_error::{opus_error, vorbis_error, wem_read_error};
use crate::{ConversionError, Sound};

pub(super) enum WavWrite {
    Linear(LinearWavWrite),
    Opus(WemOpusWavEvidence),
    Vorbis(WemVorbisWavEvidence),
}

impl WavWrite {
    pub(super) const fn report(&self) -> WavReport {
        match self {
            Self::Linear(written) => written.report,
            Self::Opus(written) => written.report,
            Self::Vorbis(written) => written.report,
        }
    }

    pub(super) const fn output_audio_start(&self) -> u64 {
        match self {
            Self::Linear(written) => written.output_audio_start,
            Self::Opus(written) => written.output.samples.start,
            Self::Vorbis(written) => written.output.samples.start,
        }
    }

    pub(super) fn linear(
        sound: &Sound,
        out: &Path,
        mut writer: &mut dyn Write,
    ) -> Result<Self, ConversionError> {
        sound
            .wem()
            .write_linear_wav(&mut writer)
            .map(Self::Linear)
            .map_err(|source| sound.audio_error(out, source))
    }

    pub(super) fn convert(
        sound: &Sound,
        out: &Path,
        writer: &mut dyn Write,
    ) -> Result<Self, ConversionError> {
        let codec = sound.wem().codec();
        if !matches!(codec, Codec::Opus | Codec::Vorbis) {
            return Self::linear(sound, out, writer);
        }
        let input = |source| wem_read_error(sound, out, source);
        let mut source = sound.wem().seekable_source().map_err(input)?;
        let mut inspected = source.inspect().map_err(input)?;
        match codec {
            Codec::Opus => inspected
                .opus_wav()
                .map_err(input)?
                .write_with_evidence(writer)
                .map(Self::Opus)
                .map_err(|source| opus_error(sound, out, source)),
            Codec::Vorbis => inspected
                .vorbis_wav()
                .map_err(input)?
                .write_with_evidence(writer)
                .map(Self::Vorbis)
                .map_err(|source| vorbis_error(sound, out, source)),
            _ => unreachable!("compressed WAV codec checked"),
        }
    }
}
