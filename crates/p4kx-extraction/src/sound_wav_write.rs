// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use p4kx_audio::{Codec, LinearWavWrite, WavReport, Wem, WemOpusWavEvidence, WemVorbisWavEvidence};

use crate::sound::conversion_error::SoundEncodeError;

pub(crate) enum WavWrite {
    Linear(LinearWavWrite),
    Opus(WemOpusWavEvidence),
    Vorbis(WemVorbisWavEvidence),
}

impl WavWrite {
    pub(crate) const fn report(&self) -> WavReport {
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
        wem: &Wem<impl AsRef<[u8]>>,
        mut writer: &mut dyn Write,
    ) -> Result<Self, SoundEncodeError> {
        Ok(Self::Linear(wem.write_linear_wav(&mut writer)?))
    }

    pub(crate) fn encode(
        wem: &Wem<impl AsRef<[u8]>>,
        writer: &mut dyn Write,
    ) -> Result<Self, SoundEncodeError> {
        let codec = wem.codec();
        if !matches!(codec, Codec::Opus | Codec::Vorbis) {
            return Self::linear(wem, writer);
        }
        let mut source = wem.seekable_source()?;
        let mut inspected = source.inspect()?;
        match codec {
            Codec::Opus => Ok(Self::Opus(
                inspected.opus_wav()?.write_with_evidence(writer)?,
            )),
            Codec::Vorbis => Ok(Self::Vorbis(
                inspected.vorbis_wav()?.write_with_evidence(writer)?,
            )),
            _ => unreachable!("compressed WAV codec checked"),
        }
    }
}
