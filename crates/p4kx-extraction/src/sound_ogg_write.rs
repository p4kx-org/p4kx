// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use p4kx_audio::{
    AudioError, Codec, OggOutputRanges, OggReport, Wem, WemOpusOggEvidence, WemVorbisOggEvidence,
};

use crate::sound::conversion_error::SoundEncodeError;

pub(crate) enum OggWrite {
    Opus(WemOpusOggEvidence),
    Vorbis(WemVorbisOggEvidence),
}

impl OggWrite {
    pub(crate) fn encode(
        wem: &Wem<impl AsRef<[u8]>>,
        writer: &mut dyn Write,
    ) -> Result<Self, SoundEncodeError> {
        let codec = wem.codec();
        if !matches!(codec, Codec::Opus | Codec::Vorbis) {
            return Err(AudioError::UnsupportedOggCodec(codec).into());
        }
        let mut source = wem.seekable_source()?;
        let mut inspected = source.inspect()?;
        match codec {
            Codec::Opus => Ok(Self::Opus(
                inspected.opus_ogg()?.write_with_evidence(writer)?,
            )),
            Codec::Vorbis => Ok(Self::Vorbis(
                inspected.vorbis_ogg()?.write_with_evidence(writer)?,
            )),
            _ => unreachable!("supported Ogg codec checked"),
        }
    }

    pub(crate) const fn report(&self) -> OggReport {
        match self {
            Self::Opus(written) => written.report,
            Self::Vorbis(written) => written.report,
        }
    }

    pub(super) const fn output(&self) -> &OggOutputRanges {
        match self {
            Self::Opus(written) => &written.output,
            Self::Vorbis(written) => &written.output,
        }
    }
}
