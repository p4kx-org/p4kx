// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded WEM reads and original input evidence.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use p4kx_archive::{Archive, Entry, EntryOrigin};
use p4kx_audio::{AudioError, OggReport, WavReport, Wem};
pub use p4kx_audio::{
    ByteOrder, WemChunkCursor, WemChunkEntry, WemCodecSource, WemCue, WemCuePoint, WemCueText,
    WemCueTextRegion, WemInfoSource, WemInspection, WemList, WemListItem, WemListPayload,
    WemLoopEnd, WemLoopSource, WemMetadata, WemPcmWav, WemPcmWriteError, WemReadError,
    WemSampleCountSource, WemSource, WemSourceChunk, WemTextUtf8, WemVorbisOgg, WemVorbisRanges,
    WemVorbisWriteError,
};

#[path = "wem_origin_plan.rs"]
mod wem_origin_plan;
pub use wem_origin_plan::{
    WemFormatField, WemOriginError, WemOriginField, WemOriginPlan, WemOriginSite,
};

use crate::entry_content::find_entry;
use crate::output_file::{read_error, write_error, write_replacing_protected};
use crate::source_protection::SourceProtection;
use crate::texture_parts::is_same_file;
use crate::{ConversionError, FileStamp, ObservedFileOrigin, SourceInput, UnknownEntries};

/// The largest WEM read into memory, far above the largest sound of a real Archive.
pub const MAX_SOUND_LEN: u64 = 512 << 20;

const MAX_UP_FRONT_RESERVATION: u64 = 64 << 20;

/// A Wwise WEM sound read from an Entry of an Archive or from a file on disk.
#[derive(Debug)]
pub struct Sound {
    protection: SourceProtection,
    name: String,
    file: Option<PathBuf>,
    wem: Wem<Vec<u8>>,
    original: Option<SoundInput>,
}

#[derive(Debug)]
enum SoundInput {
    Archive(EntryOrigin),
    File(ObservedFileOrigin),
}

impl Sound {
    /// Reads and parses the WEM Entry at `path`, matched with either separator and ignoring ASCII
    /// case.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::UnknownEntry`] when no Entry has that path,
    /// [`ConversionError::SoundTooLarge`] when it exceeds [`MAX_SOUND_LEN`], and another
    /// [`ConversionError`] when it cannot be read or is no valid WEM.
    pub fn from_archive(archive: &Archive, path: &str) -> Result<Self, ConversionError> {
        Self::read_archive(archive, path, |_| None)
    }

    /// Reads the selected WEM Entry and retains its original physical source.
    ///
    /// # Errors
    /// Returns the same errors as [`Self::from_archive`].
    pub fn from_archive_observed(archive: &Archive, path: &str) -> Result<Self, ConversionError> {
        Self::read_archive(archive, path, |entry| {
            archive.entry_origin(entry).map(SoundInput::Archive)
        })
    }

    fn read_archive(
        archive: &Archive,
        path: &str,
        observe: impl FnOnce(&Entry) -> Option<SoundInput>,
    ) -> Result<Self, ConversionError> {
        let protection = SourceProtection::new(Some(archive.path()));
        let entry =
            find_entry(archive, path).ok_or_else(|| UnknownEntries(vec![path.to_owned()]))?;
        let name = entry.path().to_owned();
        let original = observe(entry);
        let reader = archive.open_entry(entry)?;
        let bytes =
            read_capped(reader, entry.uncompressed_size(), MAX_SOUND_LEN).map_err(|source| {
                ConversionError::ReadEntry {
                    entry: name.clone(),
                    source,
                }
            })?;
        Self::parse(name, None, bytes, protection, original)
    }

    /// Reads and parses the WEM file at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::NoSuchFile`] when the file does not exist,
    /// [`ConversionError::SoundTooLarge`] when it exceeds [`MAX_SOUND_LEN`], and another
    /// [`ConversionError`] when it cannot be read or is no valid WEM.
    pub fn from_file(path: &Path) -> Result<Self, ConversionError> {
        Self::read_file(path, false)
    }

    /// Reads a WEM file with its original standalone identity.
    ///
    /// # Errors
    /// Returns the same errors as [`Self::from_file`].
    pub fn from_file_observed(path: &Path) -> Result<Self, ConversionError> {
        Self::read_file(path, true)
    }

    fn read_file(path: &Path, observe: bool) -> Result<Self, ConversionError> {
        let mut file = File::open(path).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                ConversionError::NoSuchFile {
                    path: path.to_path_buf(),
                }
            } else {
                read_error(path, source)
            }
        })?;
        let metadata = file.metadata().map_err(|source| read_error(path, source))?;
        let len = metadata.len();
        let name = path.display().to_string();
        let bytes = read_capped(&mut file, len, MAX_SOUND_LEN)
            .map_err(|source| read_error(path, source))?;
        let byte_length = bytes.as_ref().map_or(0, Vec::len);
        let mut sound = Self::parse(
            name,
            Some(path.to_path_buf()),
            bytes,
            SourceProtection::new(None),
            None,
        )?;
        if observe {
            sound.original = observed_file(
                path,
                &file,
                FileStamp::from_metadata(&metadata),
                byte_length,
            )
            .map_err(|source| read_error(path, source))?
            .map(SoundInput::File);
        }
        Ok(sound)
    }

    fn parse(
        name: String,
        file: Option<PathBuf>,
        bytes: Option<Vec<u8>>,
        protection: SourceProtection,
        original: Option<SoundInput>,
    ) -> Result<Self, ConversionError> {
        let Some(bytes) = bytes else {
            return Err(ConversionError::SoundTooLarge {
                sound: name,
                limit: MAX_SOUND_LEN,
            });
        };
        match Wem::parse(bytes) {
            Ok(wem) => Ok(Self {
                protection,
                name,
                file,
                wem,
                original,
            }),
            Err(source) => Err(ConversionError::Audio {
                sound: name,
                source,
            }),
        }
    }

    /// Borrows the original input of an observed WEM read.
    #[must_use]
    pub const fn source_input(&self) -> Option<SourceInput<'_>> {
        match &self.original {
            Some(SoundInput::Archive(entry)) => Some(SourceInput::Archive(entry)),
            Some(SoundInput::File(file)) => Some(SourceInput::File(file)),
            None => None,
        }
    }

    /// Returns the name of the sound: its Entry path or its path on disk.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the parsed WEM.
    #[must_use]
    pub const fn wem(&self) -> &Wem<Vec<u8>> {
        &self.wem
    }

    /// Writes the sound as a WAV file to `out`, replacing it; the file is written under a
    /// temporary name first, so a failed conversion leaves nothing behind.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::Audio`] naming the codec when it cannot be decoded,
    /// [`ConversionError::OutputIsInput`] when `out` is the WEM file itself, and
    /// [`ConversionError::Write`] when the file cannot be written.
    pub fn write_wav_to_file(&self, out: &Path) -> Result<WavReport, ConversionError> {
        self.refuse_input_as_output(out)?;
        write_replacing_protected(out, self.protection.context()?, None, |writer| {
            self.wem
                .write_wav(writer)
                .map_err(|source| self.audio_error(out, source))
        })
    }

    /// Rebuilds a Wwise Vorbis sound as an Ogg Vorbis file or an Opus sound as an Ogg Opus file at
    /// `out`, replacing it; the file is written under a temporary name first, so a failed
    /// conversion leaves nothing behind.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::Audio`] naming the codec when it is neither Wwise Vorbis nor
    /// Opus or the stream cannot be rebuilt, [`ConversionError::OutputIsInput`] when `out` is the
    /// WEM file itself, and [`ConversionError::Write`] when the file cannot be written.
    pub fn write_ogg_to_file(&self, out: &Path) -> Result<OggReport, ConversionError> {
        self.refuse_input_as_output(out)?;
        write_replacing_protected(out, self.protection.context()?, None, |writer| {
            self.wem
                .write_ogg(writer)
                .map_err(|source| self.audio_error(out, source))
        })
    }

    fn refuse_input_as_output(&self, out: &Path) -> Result<(), ConversionError> {
        if let Some(file) = &self.file
            && is_same_file(out, [file.clone()])
        {
            return Err(ConversionError::OutputIsInput {
                path: out.to_path_buf(),
            });
        }
        Ok(())
    }

    fn audio_error(&self, out: &Path, source: AudioError) -> ConversionError {
        match source {
            AudioError::Write(source) => write_error(out, source),
            source => ConversionError::Audio {
                sound: self.name.clone(),
                source,
            },
        }
    }
}

fn observed_file(
    path: &Path,
    handle: &File,
    before: FileStamp,
    byte_length: usize,
) -> io::Result<Option<ObservedFileOrigin>> {
    let absolute = std::path::absolute(path)?;
    let after = FileStamp::from_metadata(&handle.metadata()?);
    let stable = byte_length as u64 == before.size()
        && after == before
        && FileStamp::of(&absolute)? == before;
    Ok(stable.then(|| ObservedFileOrigin::new(absolute, before)))
}

/// Reads all of `reader`, or `None` when it holds more than `limit` bytes; at most
/// [`MAX_UP_FRONT_RESERVATION`] bytes are reserved before the data arrives.
pub(crate) fn read_capped(
    reader: impl Read,
    declared_len: u64,
    limit: u64,
) -> io::Result<Option<Vec<u8>>> {
    let capacity =
        usize::try_from(declared_len.min(limit).min(MAX_UP_FRONT_RESERVATION)).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    reader.take(limit + 1).read_to_end(&mut bytes)?;
    Ok((bytes.len() as u64 <= limit).then_some(bytes))
}
