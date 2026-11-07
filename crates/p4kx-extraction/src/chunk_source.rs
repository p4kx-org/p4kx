// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use p4kx_archive::{Archive, Entry};
use p4kx_chunks::{Chunk, ChunkFile, crch};
use p4kx_cryxml as cryxml;

use crate::entry_content::find_entry;
use crate::output_file::{read_error, write_error, write_replacing_protected};
use crate::path_index::PathIndex;
use crate::sound::read_capped;
use crate::source_protection::SourceProtection;
use crate::texture_parts::is_same_file;
use crate::{ConversionError, UnknownEntries};

/// The largest chunk file read into memory, far above the largest model of a real Archive.
pub const MAX_CHUNK_FILE_LEN: u64 = 1 << 30;

/// A `CryEngine` chunk file read from an Entry of an Archive or from a file on disk.
#[derive(Debug)]
pub struct ChunkSource {
    protection: SourceProtection,
    name: String,
    path: Option<PathBuf>,
    file: ChunkFile<Vec<u8>>,
}

impl ChunkSource {
    /// Reads the chunk file Entry at `path`, matched with either separator and ignoring ASCII case.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::UnknownEntry`] when no Entry has that path,
    /// [`ConversionError::ChunkFileTooLarge`] when it exceeds [`MAX_CHUNK_FILE_LEN`],
    /// [`ConversionError::ChunkFile`] when it is no valid chunk file, and another
    /// [`ConversionError`] when it cannot be read.
    pub fn from_archive(archive: &Archive, path: &str) -> Result<Self, ConversionError> {
        let entry =
            find_entry(archive, path).ok_or_else(|| UnknownEntries(vec![path.to_owned()]))?;
        Self::from_entry(archive, entry, MAX_CHUNK_FILE_LEN)
    }

    /// Reads the chunk file Entry at `path` found through `index`; see [`Self::from_archive`].
    ///
    /// # Errors
    ///
    /// As [`Self::from_archive`].
    pub fn from_index(index: &PathIndex<'_>, path: &str) -> Result<Self, ConversionError> {
        Self::from_index_limited(index, path, MAX_CHUNK_FILE_LEN)
    }

    /// Reads the chunk file Entry at `path` found through `index` like [`Self::from_index`], but
    /// refuses one larger than `limit` bytes with [`ConversionError::ChunkFileTooLarge`].
    ///
    /// # Errors
    ///
    /// As [`Self::from_archive`].
    pub fn from_index_limited(
        index: &PathIndex<'_>,
        path: &str,
        limit: u64,
    ) -> Result<Self, ConversionError> {
        let entry = index
            .find(path)
            .ok_or_else(|| UnknownEntries(vec![path.to_owned()]))?;
        Self::from_entry(index.archive(), entry, limit)
    }

    fn from_entry(archive: &Archive, entry: &Entry, limit: u64) -> Result<Self, ConversionError> {
        let protection = SourceProtection::new(Some(archive.path()));
        let name = entry.path().to_owned();
        let reader = archive.open_entry(entry)?;
        let bytes = read_capped(reader, entry.uncompressed_size(), limit).map_err(|source| {
            ConversionError::ReadEntry {
                entry: name.clone(),
                source,
            }
        })?;
        Self::parse(name, None, bytes, limit, protection)
    }

    /// Reads the chunk file at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::NoSuchFile`] when the file does not exist,
    /// [`ConversionError::ChunkFileTooLarge`] when it exceeds [`MAX_CHUNK_FILE_LEN`],
    /// [`ConversionError::ChunkFile`] when it is no valid chunk file, and another
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
        let bytes = read_capped(file, len, MAX_CHUNK_FILE_LEN)
            .map_err(|source| read_error(path, source))?;
        Self::parse(
            path.display().to_string(),
            Some(path.to_path_buf()),
            bytes,
            MAX_CHUNK_FILE_LEN,
            SourceProtection::new(None),
        )
    }

    fn parse(
        name: String,
        path: Option<PathBuf>,
        bytes: Option<Vec<u8>>,
        limit: u64,
        protection: SourceProtection,
    ) -> Result<Self, ConversionError> {
        let Some(bytes) = bytes else {
            return Err(ConversionError::ChunkFileTooLarge { file: name, limit });
        };
        match ChunkFile::parse(bytes) {
            Ok(file) => Ok(Self {
                protection,
                name,
                path,
                file,
            }),
            Err(source) => Err(ConversionError::ChunkFile { file: name, source }),
        }
    }

    /// Returns the name of the chunk file: its Entry path or its path on disk.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the parsed chunk file.
    #[must_use]
    pub const fn file(&self) -> &ChunkFile<Vec<u8>> {
        &self.file
    }

    pub(crate) fn into_protection(self) -> SourceProtection {
        self.protection
    }

    fn chunk_and_data(&self, index: usize, out: &Path) -> Result<(&Chunk, &[u8]), ConversionError> {
        let chunk_and_data = self
            .file
            .chunks()
            .get(index)
            .zip(self.file.chunk_data(index));
        let found = chunk_and_data.ok_or_else(|| ConversionError::NoSuchChunk {
            file: self.name.clone(),
            index,
            count: self.file.chunks().len(),
        })?;
        if let Some(path) = &self.path
            && is_same_file(out, [path.clone()])
        {
            return Err(ConversionError::OutputIsInput {
                path: out.to_path_buf(),
            });
        }
        Ok(found)
    }

    /// Writes the data of the chunk at `index` in the table unchanged to `out`, replacing it, and
    /// returns the chunk; the file is written under a temporary name first.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::NoSuchChunk`] when the table has no chunk at `index`,
    /// [`ConversionError::OutputIsInput`] when `out` is the chunk file itself, and
    /// [`ConversionError::Write`] when the file cannot be written.
    pub fn write_chunk_to_file(&self, index: usize, out: &Path) -> Result<Chunk, ConversionError> {
        let (chunk, data) = self.chunk_and_data(index, out)?;
        write_replacing_protected(out, self.protection.context()?, None, |writer| {
            writer
                .write_all(data)
                .map_err(|source| write_error(out, source))?;
            Ok(())
        })?;
        Ok(*chunk)
    }

    /// Writes the chunk at `index` as UTF-8 text to `out`, replacing it, and returns the chunk:
    /// a `CryXmlB` chunk (`CrCh` types `0x0004`, `0x0014`) as XML, a `Json` or `ModelPaths` chunk
    /// (`0x0011`, `0x0013`) as it is stored; text is checked to be UTF-8 first.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::NoSuchChunk`] when the table has no chunk at `index`,
    /// [`ConversionError::ChunkNotText`] when the chunk's type has no text form,
    /// [`ConversionError::ChunkText`] when its data is no valid `CryXml` or UTF-8 text,
    /// [`ConversionError::OutputIsInput`] when `out` is the chunk file itself, and
    /// [`ConversionError::Write`] when the file cannot be written.
    pub fn write_chunk_text_to_file(
        &self,
        index: usize,
        out: &Path,
    ) -> Result<Chunk, ConversionError> {
        let (chunk, data) = self.chunk_and_data(index, out)?;
        let kind = chunk.chunk_type();
        let text_error = |source| ConversionError::ChunkText {
            file: self.name.clone(),
            index,
            source,
        };
        let utf8 = |bytes| {
            std::str::from_utf8(bytes).map_err(|error| text_error(ChunkTextError::Utf8(error)))
        };
        let text = if kind == crch::CRY_XML_B || kind == crch::CRY_XML_B_2 {
            match cryxml::read(data).map_err(|error| text_error(ChunkTextError::CryXml(error)))? {
                cryxml::Xml::Text(bytes) => ChunkText::Utf8(utf8(bytes)?),
                xml @ cryxml::Xml::CryXml(_) => ChunkText::Xml(xml),
            }
        } else if kind == crch::JSON || kind == crch::MODEL_PATHS {
            ChunkText::Utf8(utf8(data)?)
        } else {
            return Err(ConversionError::ChunkNotText {
                file: self.name.clone(),
                index,
                chunk_type: kind.to_string(),
            });
        };
        write_replacing_protected(out, self.protection.context()?, None, |writer| {
            match text {
                ChunkText::Xml(xml) => xml.write_to(&mut *writer),
                ChunkText::Utf8(text) => writer.write_all(text.as_bytes()),
            }
            .map_err(|source| write_error(out, source))
        })?;
        Ok(*chunk)
    }
}

enum ChunkText<'a> {
    Xml(cryxml::Xml<'a>),
    Utf8(&'a str),
}

/// Why the data of a chunk has no text form.
#[derive(Debug, thiserror::Error)]
pub enum ChunkTextError {
    /// The data is no readable `CryXml`.
    #[error("the data is not valid CryXml")]
    CryXml(#[source] cryxml::CryXmlError),
    /// The data is no UTF-8 text.
    #[error("the data is not UTF-8 text")]
    Utf8(#[source] std::str::Utf8Error),
}
