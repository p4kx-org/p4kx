// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_audio::{AudioError, WemOpusWriteError, WemReadError, WemVorbisWriteError};

use crate::output_file::write_error;
use crate::{ConversionError, Sound};

#[derive(Debug, thiserror::Error)]
pub(crate) enum SoundEncodeError {
    #[error(transparent)]
    Audio(#[from] AudioError),
    #[error(transparent)]
    Input(#[from] WemReadError),
    #[error(transparent)]
    Opus(#[from] WemOpusWriteError),
    #[error(transparent)]
    Vorbis(#[from] WemVorbisWriteError),
}

impl SoundEncodeError {
    pub(super) fn into_conversion_error(self, sound: &Sound, out: &Path) -> ConversionError {
        match self {
            Self::Audio(cause) => sound.audio_error(out, cause),
            Self::Input(cause) => wem_read_error(sound, out, cause),
            Self::Opus(cause) => opus_error(sound, out, cause),
            Self::Vorbis(cause) => vorbis_error(sound, out, cause),
        }
    }

    pub(crate) fn into_audio_error(self) -> AudioError {
        match self {
            Self::Audio(error)
            | Self::Input(WemReadError::Format(error))
            | Self::Opus(WemOpusWriteError::Input(WemReadError::Format(error)))
            | Self::Vorbis(WemVorbisWriteError::Format(error)) => error,
            Self::Opus(WemOpusWriteError::Output(error))
            | Self::Vorbis(WemVorbisWriteError::Output(error)) => AudioError::Write(error),
            other => AudioError::Write(io::Error::other(other)),
        }
    }
}

fn wem_read_error(sound: &Sound, out: &Path, error: WemReadError) -> ConversionError {
    match error {
        WemReadError::Format(source) => sound.audio_error(out, source),
        WemReadError::Io(source) => replay_read_error(sound, source),
        source => replay_read_error(sound, io::Error::other(source)),
    }
}

fn opus_error(sound: &Sound, out: &Path, error: WemOpusWriteError) -> ConversionError {
    match error {
        WemOpusWriteError::Input(source) => wem_read_error(sound, out, source),
        WemOpusWriteError::Output(source) => write_error(out, source),
        source => replay_read_error(sound, io::Error::other(source)),
    }
}

fn vorbis_error(sound: &Sound, out: &Path, error: WemVorbisWriteError) -> ConversionError {
    match error {
        WemVorbisWriteError::Format(source) => sound.audio_error(out, source),
        WemVorbisWriteError::Input(source) => replay_read_error(sound, source),
        WemVorbisWriteError::Output(source) => write_error(out, source),
        source => replay_read_error(sound, io::Error::other(source)),
    }
}

fn replay_read_error(sound: &Sound, source: io::Error) -> ConversionError {
    ConversionError::ReadEntry {
        entry: sound.name().to_owned(),
        source,
    }
}
