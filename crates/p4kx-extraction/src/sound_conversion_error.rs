// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_audio::{WemOpusWriteError, WemReadError, WemVorbisWriteError};

use crate::output_file::write_error;
use crate::{ConversionError, Sound};

pub(super) fn wem_read_error(sound: &Sound, out: &Path, error: WemReadError) -> ConversionError {
    match error {
        WemReadError::Format(source) => sound.audio_error(out, source),
        WemReadError::Io(source) => replay_read_error(sound, source),
        source => replay_read_error(sound, io::Error::other(source)),
    }
}

pub(super) fn opus_error(sound: &Sound, out: &Path, error: WemOpusWriteError) -> ConversionError {
    match error {
        WemOpusWriteError::Input(source) => wem_read_error(sound, out, source),
        WemOpusWriteError::Output(source) => write_error(out, source),
        source => replay_read_error(sound, io::Error::other(source)),
    }
}

pub(super) fn vorbis_error(
    sound: &Sound,
    out: &Path,
    error: WemVorbisWriteError,
) -> ConversionError {
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
