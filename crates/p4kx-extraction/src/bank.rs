// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded soundbank reads, source inspection, and extraction.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use p4kx_archive::Archive;
use p4kx_audio::{AudioError, EmbeddedWem, MediaClass, Soundbank};
pub use p4kx_audio::{
    BankActionFields, BankActionRead, BankContainerChild, BankContainerChildren,
    BankContainerFields, BankContainerRead, BankDeclaredLengthSource, BankDidx, BankDidxEntry,
    BankEvent, BankEventAction, BankEventCountEncoding, BankEventRead, BankHirc, BankHircAction,
    BankHircContainer, BankHircEvent, BankHircField, BankHircObject, BankHircSemantic,
    BankHircSound, BankMediaClassSource, BankMediaInfo, BankNodeParentFields, BankObservedHeader,
    BankPlayBankDecision, BankPlayBankFields, BankReadError, BankSectionCursor,
    BankSoundContinuation, BankSoundFields, BankSoundNodeBase, BankSoundNodeFields, BankSoundRead,
    BankSource, BankSourceSection, BankStid, BankStidName, ContainerKind, MediaStorage,
    SoundSource,
};
use rayon::prelude::*;

use crate::entry_content::find_entry;
use crate::output_file::{read_error, write_error, write_replacing_protected};
use crate::sound::read_capped;
use crate::source_protection::SourceProtection;
use crate::{ConversionError, UnknownEntries};

/// The largest soundbank read into memory, far above the largest soundbank of a real Archive.
pub const MAX_SOUNDBANK_LEN: u64 = 512 << 20;

/// The file format [`Bank::extract_wems`] writes each embedded WEM in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WemFormat {
    /// The WEM bytes unchanged, as `<id>.wem`; a prefetched start is written as the bank holds it.
    Wem,
    /// A 16-bit PCM WAV, as `<id>.wav`; codecs without a decoder and prefetched starts fail per WEM.
    Wav,
    /// An Ogg Vorbis or Ogg Opus file rebuilt from a Wwise Vorbis or Opus WEM, as `<id>.ogg`;
    /// other codecs and prefetched starts fail per WEM.
    Ogg,
}

impl WemFormat {
    /// Returns the file extension, without the dot.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Wem => "wem",
            Self::Wav => "wav",
            Self::Ogg => "ogg",
        }
    }
}

/// One embedded WEM [`Bank::extract_wems`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenWem {
    /// The WEM ID.
    pub id: u32,
    /// The file written.
    pub path: PathBuf,
    /// Its size in bytes.
    pub size: u64,
    /// Whether it is only the prefetched start of a streamed WEM, written as the bank holds it.
    pub prefetch: bool,
}

/// One embedded WEM [`Bank::extract_wems`] could not write.
#[derive(Debug)]
pub struct WemFailure {
    /// The WEM ID.
    pub id: u32,
    /// Why.
    pub error: ConversionError,
}

/// What [`Bank::extract_wems`] did, both lists in ascending ID order.
#[derive(Debug, Default)]
pub struct BankExtraction {
    /// The WEMs written.
    pub written: Vec<WrittenWem>,
    /// The WEMs that failed.
    pub failures: Vec<WemFailure>,
}

/// A Wwise BNK soundbank read from an Entry of an Archive or from a file on disk.
#[derive(Debug)]
pub struct Bank {
    protection: SourceProtection,
    name: String,
    soundbank: Soundbank<Vec<u8>>,
}

impl Bank {
    /// Reads and parses the soundbank Entry at `path`, matched with either separator and ignoring
    /// ASCII case.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::UnknownEntry`] when no Entry has that path,
    /// [`ConversionError::SoundTooLarge`] when it exceeds [`MAX_SOUNDBANK_LEN`],
    /// [`ConversionError::Soundbank`] when it is no valid soundbank, and another
    /// [`ConversionError`] when it cannot be read.
    pub fn from_archive(archive: &Archive, path: &str) -> Result<Self, ConversionError> {
        let protection = SourceProtection::new(Some(archive.path()));
        let entry =
            find_entry(archive, path).ok_or_else(|| UnknownEntries(vec![path.to_owned()]))?;
        let name = entry.path().to_owned();
        let reader = archive.open_entry(entry)?;
        let bytes = read_capped(reader, entry.uncompressed_size(), MAX_SOUNDBANK_LEN).map_err(
            |source| ConversionError::ReadEntry {
                entry: name.clone(),
                source,
            },
        )?;
        Self::parse(name, bytes, protection)
    }

    /// Reads and parses the soundbank file at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::NoSuchFile`] when the file does not exist,
    /// [`ConversionError::SoundTooLarge`] when it exceeds [`MAX_SOUNDBANK_LEN`],
    /// [`ConversionError::Soundbank`] when it is no valid soundbank, and another
    /// [`ConversionError`] when it cannot be read.
    pub fn from_file(path: &Path) -> Result<Self, ConversionError> {
        let file = File::open(path).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                ConversionError::NoSuchFile {
                    path: path.to_path_buf(),
                }
            } else {
                read_error(path, source)
            }
        })?;
        let len = file
            .metadata()
            .map_err(|source| read_error(path, source))?
            .len();
        let bytes =
            read_capped(file, len, MAX_SOUNDBANK_LEN).map_err(|source| read_error(path, source))?;
        Self::parse(
            path.display().to_string(),
            bytes,
            SourceProtection::new(None),
        )
    }

    fn parse(
        name: String,
        bytes: Option<Vec<u8>>,
        protection: SourceProtection,
    ) -> Result<Self, ConversionError> {
        let Some(bytes) = bytes else {
            return Err(ConversionError::SoundTooLarge {
                sound: name,
                limit: MAX_SOUNDBANK_LEN,
            });
        };
        match Soundbank::parse(bytes) {
            Ok(soundbank) => Ok(Self {
                protection,
                name,
                soundbank,
            }),
            Err(source) => Err(ConversionError::Soundbank { bank: name, source }),
        }
    }

    /// Returns the name of the soundbank: its Entry path or its path on disk.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the parsed soundbank.
    #[must_use]
    pub const fn soundbank(&self) -> &Soundbank<Vec<u8>> {
        &self.soundbank
    }

    /// Writes every embedded WEM into `folder` as `<id>.<extension>` in parallel, replacing
    /// existing files; a WEM that fails leaves no file and does not stop the others.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::Write`] when `folder` cannot be created.
    pub fn extract_wems(
        &self,
        folder: &Path,
        format: WemFormat,
    ) -> Result<BankExtraction, ConversionError> {
        crate::check_protected_output(folder, self.protection.context()?, None)?;
        fs::create_dir_all(folder).map_err(|source| write_error(folder, source))?;
        let wems: Vec<EmbeddedWem<'_>> = self.soundbank.wems().collect();
        let outcomes: Vec<(u32, Result<WrittenWem, ConversionError>)> = wems
            .par_iter()
            .map(|wem| (wem.id(), self.write_wem(wem, folder, format)))
            .collect();
        let mut extraction = BankExtraction::default();
        for (id, outcome) in outcomes {
            match outcome {
                Ok(written) => extraction.written.push(written),
                Err(error) => extraction.failures.push(WemFailure { id, error }),
            }
        }
        Ok(extraction)
    }

    fn write_wem(
        &self,
        wem: &EmbeddedWem<'_>,
        folder: &Path,
        format: WemFormat,
    ) -> Result<WrittenWem, ConversionError> {
        if format != WemFormat::Wem && wem.is_prefetch() {
            return Err(ConversionError::PrefetchedWem {
                bank: self.name.clone(),
                id: wem.id(),
            });
        }
        let extension = if format == WemFormat::Wem && wem.class() == MediaClass::Midi {
            "mid"
        } else {
            format.extension()
        };
        let path = folder.join(format!("{}.{}", wem.id(), extension));
        let audio_error = |source| match source {
            AudioError::Write(source) => write_error(&path, source),
            source => ConversionError::Audio {
                sound: format!("{} WEM {}", self.name, wem.id()),
                source,
            },
        };
        let size =
            write_replacing_protected(
                &path,
                self.protection.context()?,
                None,
                |writer| match format {
                    WemFormat::Wem => {
                        writer
                            .write_all(wem.bytes())
                            .map_err(|source| write_error(&path, source))?;
                        Ok(wem.bytes().len() as u64)
                    }
                    WemFormat::Wav => wem
                        .parse()
                        .and_then(|parsed| parsed.write_wav(writer))
                        .map(|report| report.size)
                        .map_err(audio_error),
                    WemFormat::Ogg => wem
                        .parse()
                        .and_then(|parsed| parsed.write_ogg(writer))
                        .map(|report| report.size)
                        .map_err(audio_error),
                },
            )?;
        Ok(WrittenWem {
            id: wem.id(),
            path,
            size,
            prefetch: wem.is_prefetch(),
        })
    }
}
