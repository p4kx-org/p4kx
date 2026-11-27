// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::io::Read;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex, OnceLock};

use p4kx_archive::{Archive, Entry};
use thiserror::Error;

use crate::bounded_read_cache::BoundedReadCache;
use crate::loadout_file_read::{self, LoadoutFileBytes};
use crate::{LoadoutFileIdentity, LoadoutFileRead, LoadoutFileSource};

/// The largest XML Loadout file that is read, 16 MiB.
pub const MAX_XML_LOADOUT_LEN: u64 = 16 << 20;

/// Where the XML Loadout files a Record names are read from.
pub trait LoadoutFiles: Sync {
    /// Returns the bytes of the file at the configured source path.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the file is absent or cannot be read.
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError>;

    /// Reads once with an unbound source unless this provider captures its original admission.
    fn read_observed(&self, path: &str) -> LoadoutFileRead {
        LoadoutFileRead::unbound(path, self.read(path))
    }

    /// Reports whether this provider supplies vehicle definitions and patches.
    fn reads_vehicle_implementations(&self) -> bool {
        true
    }

    /// Returns the exact source identity selected by the same resolution as `read`.
    fn identity(&self, _path: &str) -> Option<LoadoutFileIdentity<'_>> {
        None
    }
}

/// An XML Loadout file cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum LoadoutFileError {
    /// The optional observer stopped the operation.
    #[error("the loadout file observer stopped the operation")]
    ObservationStopped,
    /// Original Entry membership could not be retained for an admitted source.
    #[error("the original loadout file admission could not be retained")]
    SourceEvidenceUnavailable,
    /// There is no such file.
    #[error("there is no such file")]
    NotFound,
    /// Several physical Entries have the requested name.
    #[error("several physical Entries match the XML Loadout")]
    Ambiguous,
    /// The only matching physical Entry is outside the selected inputs.
    #[error("the XML Loadout is outside the selected inputs")]
    NotSelected,
    /// The original source inventory or the selected name cannot establish membership.
    #[error("the XML Loadout selection cannot establish original membership")]
    UnprovedSelection,
    /// The file is larger than [`MAX_XML_LOADOUT_LEN`].
    #[error("the file holds {size} bytes, more than the limit of {MAX_XML_LOADOUT_LEN} bytes")]
    TooLarge {
        /// Its size.
        size: u64,
    },
    /// The file cannot be read.
    #[error("{reason}")]
    Unreadable {
        /// Why.
        reason: String,
    },
}

/// Reads XML Loadout files from the Entries of an Archive below `Data\`.
#[derive(Debug)]
pub struct ArchiveFiles<'a> {
    archive: &'a Archive,
    index: OnceLock<HashMap<String, usize>>,
    verify_entries: bool,
    xml_read_cache: Option<Mutex<XmlReadCache>>,
}

type XmlReadCache = BoundedReadCache<XmlReadKey, Arc<Vec<u8>>>;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct XmlReadKey {
    entry_index: usize,
    key_generation: u64,
    verified: bool,
}

impl<'a> ArchiveFiles<'a> {
    /// Reads from `archive`; its XML Entries are indexed on the first read.
    #[must_use]
    pub const fn new(archive: &'a Archive) -> Self {
        Self {
            archive,
            index: OnceLock::new(),
            verify_entries: false,
            xml_read_cache: None,
        }
    }

    /// Retains successful observed XML reads for this opened Archive.
    /// The budget covers retained Entry count and allocated decoded byte capacity.
    #[must_use]
    pub fn with_xml_read_cache(mut self, max_entries: NonZeroUsize, max_bytes: usize) -> Self {
        self.xml_read_cache = Some(Mutex::new(BoundedReadCache::new(max_entries, max_bytes)));
        self
    }

    /// Verifies each selected document's checksums and decoded extent while reading it once.
    #[must_use]
    pub const fn with_verified_entries(mut self) -> Self {
        self.verify_entries = true;
        self
    }

    /// Reports whether selected documents are verified as they are read.
    #[must_use]
    pub const fn verifies_entries(&self) -> bool {
        self.verify_entries
    }

    /// Borrows the Archive supplying these files.
    #[must_use]
    pub const fn archive(&self) -> &'a Archive {
        self.archive
    }

    fn index(&self) -> &HashMap<String, usize> {
        self.index.get_or_init(|| {
            self.archive
                .entries()
                .iter()
                .enumerate()
                .filter(|(_, entry)| may_have_xml_extension(entry.path()))
                .map(|(index, entry)| (key(entry.path()), index))
                .filter(|(key, _)| {
                    std::path::Path::new(key)
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
                })
                .collect()
        })
    }

    fn entry_index(&self, path: &str) -> Option<usize> {
        let mut wanted = key(path);
        if !wanted.starts_with("data\\") {
            wanted.insert_str(0, "data\\");
        }
        self.index().get(&wanted).copied()
    }
}

impl LoadoutFiles for ArchiveFiles<'_> {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        let entry = self
            .entry_index(path)
            .and_then(|index| self.archive.entries().get(index))
            .ok_or(LoadoutFileError::NotFound)?;
        self.read_entry(entry)
    }

    fn read_observed(&self, path: &str) -> LoadoutFileRead {
        let Some(entry) = self
            .entry_index(path)
            .and_then(|index| self.archive.entries().get(index))
        else {
            return LoadoutFileRead::new(
                LoadoutFileSource::missing_archive(path, self.archive.input_origin()),
                Err(LoadoutFileError::NotFound),
            );
        };
        let Some(original) = self.archive.entry_origin(entry) else {
            return LoadoutFileRead::unbound(
                path,
                Err(LoadoutFileError::SourceEvidenceUnavailable),
            );
        };
        let outcome = self.read_observed_entry(entry, original.entry().index());
        LoadoutFileRead::with_bytes(LoadoutFileSource::archive(original), outcome)
    }

    fn identity(&self, path: &str) -> Option<LoadoutFileIdentity<'_>> {
        let index = self.entry_index(path)?;
        let entry = self.archive.entries().get(index)?;
        Some(LoadoutFileIdentity {
            entry_index: Some(index),
            path: entry.path(),
        })
    }
}

impl ArchiveFiles<'_> {
    fn read_observed_entry(
        &self,
        entry: &Entry,
        entry_index: usize,
    ) -> Result<LoadoutFileBytes, LoadoutFileError> {
        let Some(cache) = &self.xml_read_cache else {
            return self.read_entry(entry).map(LoadoutFileBytes::Owned);
        };
        let key = XmlReadKey {
            entry_index,
            key_generation: self.archive.key_generation(),
            verified: self.verify_entries,
        };
        if let Some(bytes) = cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(key).map(Arc::clone))
            && self.archive.key_generation() == key.key_generation
        {
            return Ok(LoadoutFileBytes::Shared(bytes));
        }
        let bytes = self.read_entry(entry)?;
        if let Ok(mut cache) = cache.lock()
            && self.archive.key_generation() == key.key_generation
        {
            let bytes = Arc::new(bytes);
            cache.insert(key, bytes.capacity(), Arc::clone(&bytes));
            return Ok(LoadoutFileBytes::Shared(bytes));
        }
        Ok(LoadoutFileBytes::Owned(bytes))
    }

    fn read_entry(&self, entry: &Entry) -> Result<Vec<u8>, LoadoutFileError> {
        if self.verify_entries {
            return loadout_file_read::verified_bytes(self.archive, entry, &|| false);
        }
        let size = entry.uncompressed_size();
        if size > MAX_XML_LOADOUT_LEN {
            return Err(LoadoutFileError::TooLarge { size });
        }
        let unreadable = |reason: String| LoadoutFileError::Unreadable { reason };
        let reader = self
            .archive
            .open_entry(entry)
            .map_err(|error| unreadable(error.to_string()))?;
        let mut bytes = Vec::new();
        reader
            .take(MAX_XML_LOADOUT_LEN)
            .read_to_end(&mut bytes)
            .map_err(|error| unreadable(error.to_string()))?;
        Ok(bytes)
    }
}

fn may_have_xml_extension(path: &str) -> bool {
    path.rsplit(['/', '\\'])
        .find(|part| !part.is_empty() && *part != ".")
        .and_then(|part| part.as_bytes().last_chunk::<4>())
        .is_some_and(|suffix| suffix.eq_ignore_ascii_case(b".xml"))
}

/// Returns a path in lower case with backslash separators and no leading separator.
fn key(path: &str) -> String {
    path.trim_start_matches(['/', '\\'])
        .chars()
        .map(|c| {
            if c == '/' {
                '\\'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "archive_xml_read_cache_tests.rs"]
mod tests;
