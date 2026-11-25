// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;
use std::path::Path;

use p4kx_audio::{
    AudioError, Codec, OggOutputRanges, OggReport, WemOpusOggEvidence, WemVorbisOggEvidence,
};

use crate::sound::conversion_error::{opus_error, vorbis_error, wem_read_error};
use crate::{ConversionError, Sound};

pub(super) enum OggWrite {
    Opus(WemOpusOggEvidence),
    Vorbis(WemVorbisOggEvidence),
}

impl OggWrite {
    pub(super) fn convert(
        sound: &Sound,
        out: &Path,
        writer: &mut dyn Write,
    ) -> Result<Self, ConversionError> {
        let codec = sound.wem().codec();
        if !matches!(codec, Codec::Opus | Codec::Vorbis) {
            return Err(sound.audio_error(out, AudioError::UnsupportedOggCodec(codec)));
        }
        let input = |source| wem_read_error(sound, out, source);
        let mut source = sound.wem().seekable_source().map_err(input)?;
        let mut inspected = source.inspect().map_err(input)?;
        match codec {
            Codec::Opus => inspected
                .opus_ogg()
                .map_err(input)?
                .write_with_evidence(writer)
                .map(Self::Opus)
                .map_err(|source| opus_error(sound, out, source)),
            Codec::Vorbis => inspected
                .vorbis_ogg()
                .map_err(input)?
                .write_with_evidence(writer)
                .map(Self::Vorbis)
                .map_err(|source| vorbis_error(sound, out, source)),
            _ => unreachable!("supported Ogg codec checked"),
        }
    }

    pub(super) const fn report(&self) -> OggReport {
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
