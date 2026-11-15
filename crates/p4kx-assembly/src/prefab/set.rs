// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;

use p4kx_archive::{Archive, Entry};
use p4kx_model::{
    PositionedPrefabLibrary, PrefabLibrary, PrefabLibrarySource, PrefabParseFailure,
    PrefabParseMode,
};

use super::{MAX_PREFAB_LIBRARY_LEN, PrefabError, PrefabUnreadableSource};

mod nested;
pub(super) use nested::Lookup;

const FOLDER: &str = "data/prefabs/";
const EXTENSION: &str = ".xml";

/// A prefab library read from the Archive.
#[derive(Debug)]
pub struct PrefabFile {
    slot: usize,
    name: String,
    library: PrefabLibrary,
    source: Option<PrefabLibrarySource>,
    by_name: HashMap<String, usize>,
}

impl PrefabFile {
    /// Returns the name of the library file, as `PrefabSet::library` takes it: the path below
    /// `Data\Prefabs` without `.xml`, with `/` as separator.
    #[must_use]
    pub fn file_name(&self) -> &str {
        &self.name
    }

    /// Returns the library.
    #[must_use]
    pub const fn library(&self) -> &PrefabLibrary {
        &self.library
    }

    /// Returns original XML sites retained by the library parser.
    #[must_use]
    pub const fn source(&self) -> Option<&PrefabLibrarySource> {
        self.source.as_ref()
    }

    pub(super) const fn slot(&self) -> usize {
        self.slot
    }

    pub(super) fn find(&self, name: &str) -> Option<usize> {
        self.by_name.get(&name.to_ascii_lowercase()).copied()
    }
}

#[derive(Debug)]
struct Slot<'archive> {
    key: String,
    shown: String,
    entry: &'archive Entry,
}

type GuidIndex = HashMap<String, Vec<(usize, usize)>>;

/// The prefab libraries below `Data\Prefabs` of an Archive, read when first needed.
///
/// A library is named like its file: the path below `Data\Prefabs` without `.xml`, ignoring case
/// and with `\` or `/` as separator, e.g. `pu/derelict/RSI/Constellation`. A nested prefab is
/// found by the `PrefabGUID` of the object that instances it, the `Id` of the prefab, which is
/// how the editor finds it. The `PrefabName` of the object, the library name and the prefab name
/// joined by `.`, is tried first because it needs only one library; when it finds no prefab of
/// that `Id`, the `Id` is looked up in an index of every library, read once, four at a time, each dropped once its `Id`s are indexed.
#[derive(Debug)]
pub struct PrefabSet<'a> {
    archive: &'a Archive,
    slots: Vec<Slot<'a>>,
    files: HashMap<String, usize>,
    loaded: HashMap<usize, Arc<PrefabFile>>,
    failed: HashMap<usize, FailedRead>,
    guids: Option<GuidIndex>,
    capture: PrefabCapture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrefabCapture {
    Library,
    Objects,
}

fn normalised(path: &str) -> String {
    path.trim_start_matches(['/', '\\'])
        .chars()
        .map(|c| {
            if c == '\\' {
                '/'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

enum Failure {
    Fatal(Box<dyn std::error::Error + Send + Sync>),
    Unreadable(FailedRead),
}

#[derive(Debug)]
struct FailedRead {
    reason: String,
    source: Option<PrefabUnreadableSource>,
}

impl Failure {
    fn unreadable(reason: impl Into<String>, source: Option<PrefabUnreadableSource>) -> Self {
        Self::Unreadable(FailedRead {
            reason: reason.into(),
            source,
        })
    }

    fn parsed(error: PrefabParseFailure) -> Self {
        let reason = error.to_string();
        let source = error.into_diagnostic().map(PrefabUnreadableSource::Payload);
        Self::unreadable(reason, source)
    }
}

impl<'a> PrefabSet<'a> {
    /// Indexes the libraries of `archive`; none is read yet.
    #[must_use]
    pub fn new(archive: &'a Archive) -> Self {
        Self::build(archive, PrefabCapture::Library)
    }

    /// Indexes the Archive and retains object source positions when a library is read.
    #[must_use]
    pub fn with_object_sources(archive: &'a Archive) -> Self {
        Self::build(archive, PrefabCapture::Objects)
    }

    pub(super) fn captures_objects(&self) -> bool {
        self.capture == PrefabCapture::Objects
    }

    fn build(archive: &'a Archive, capture: PrefabCapture) -> Self {
        let mut slots: Vec<Slot<'_>> = archive
            .entries()
            .iter()
            .filter_map(|entry| {
                let path = entry.path().trim_start_matches(['/', '\\']);
                let key = normalised(path);
                let name = key.strip_prefix(FOLDER)?.strip_suffix(EXTENSION)?;
                let shown = path.get(FOLDER.len()..path.len().checked_sub(EXTENSION.len())?)?;
                (!name.is_empty()).then(|| Slot {
                    key: name.to_owned(),
                    shown: shown.replace('\\', "/"),
                    entry,
                })
            })
            .collect();
        slots.sort_by(|a, b| {
            a.key
                .cmp(&b.key)
                .then(a.entry.index().cmp(&b.entry.index()))
        });
        let mut files = HashMap::with_capacity(slots.len());
        for (slot, found) in slots.iter().enumerate() {
            files.entry(found.key.clone()).or_insert(slot);
        }
        Self {
            archive,
            slots,
            files,
            loaded: HashMap::new(),
            failed: HashMap::new(),
            guids: None,
            capture,
        }
    }

    /// Returns the names of the libraries, sorted ignoring case, with `/` as separator and the
    /// case the Archive spells them in.
    pub fn libraries(&self) -> impl ExactSizeIterator<Item = &str> {
        self.library_entries().map(|(name, _)| name)
    }

    /// Returns each displayed library name and its physical directory Entry.
    pub fn library_entries(&self) -> impl ExactSizeIterator<Item = (&str, &'a Entry)> {
        self.slots
            .iter()
            .map(|slot| (slot.shown.as_str(), slot.entry))
    }

    /// Returns the Entry that supplied a library loaded by this set.
    #[must_use]
    pub fn source_entry(&self, file: &PrefabFile) -> Option<&'a Entry> {
        let loaded = self.loaded.get(&file.slot)?;
        if !std::ptr::eq(file, loaded.as_ref()) {
            return None;
        }
        self.slots.get(file.slot).map(|slot| slot.entry)
    }

    fn slot_of(&self, name: &str) -> Option<usize> {
        let key = normalised(name);
        let key = key.strip_prefix(FOLDER).unwrap_or(&key);
        self.files
            .get(key)
            .or_else(|| self.files.get(key.strip_suffix(EXTENSION)?))
            .copied()
    }

    /// Returns whether the Archive holds the library `name`.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.slot_of(name).is_some()
    }

    /// Reads the library `name`, once.
    ///
    /// # Errors
    ///
    /// Returns [`PrefabError`] when the Archive holds no such library or it cannot be read.
    pub fn library(&mut self, name: &str) -> Result<Arc<PrefabFile>, PrefabError> {
        let Some(slot) = self.slot_of(name) else {
            return Err(PrefabError::NoSuchLibrary {
                name: name.to_owned(),
            });
        };
        self.load(slot)
    }

    pub(super) fn shown(&self, slot: usize) -> String {
        self.slots
            .get(slot)
            .map_or_else(String::new, |found| found.shown.clone())
    }

    fn load(&mut self, slot: usize) -> Result<Arc<PrefabFile>, PrefabError> {
        if let Some(file) = self.loaded.get(&slot) {
            return Ok(Arc::clone(file));
        }
        if let Some(failure) = self.failed.get(&slot) {
            return Err(PrefabError::Unreadable {
                library: self.shown(slot),
                reason: failure.reason.clone(),
            });
        }
        let mode = match self.capture {
            PrefabCapture::Library => PrefabParseMode::Positions,
            PrefabCapture::Objects => PrefabParseMode::Objects,
        };
        match self.read(slot, |bytes| PrefabLibrary::parse_observed(bytes, mode)) {
            Ok(parsed) => {
                let file = Arc::new(file_of(slot, self.shown(slot), parsed));
                self.loaded.insert(slot, Arc::clone(&file));
                Ok(file)
            }
            Err(Failure::Fatal(error)) => Err(PrefabError::Archive(error)),
            Err(Failure::Unreadable(failure)) => {
                let reason = failure.reason.clone();
                self.failed.insert(slot, failure);
                Err(PrefabError::Unreadable {
                    library: self.shown(slot),
                    reason,
                })
            }
        }
    }

    fn read<T>(
        &self,
        slot: usize,
        parse: impl FnOnce(&[u8]) -> Result<T, PrefabParseFailure>,
    ) -> Result<T, Failure> {
        let entry = self
            .slots
            .get(slot)
            .map(|found| found.entry)
            .ok_or_else(|| {
                Failure::unreadable(
                    "the Entry is gone",
                    Some(PrefabUnreadableSource::Unavailable),
                )
            })?;
        let size = entry.uncompressed_size();
        if size > MAX_PREFAB_LIBRARY_LEN {
            return Err(Failure::unreadable(
                format!("it holds {size} bytes, more than the limit of {MAX_PREFAB_LIBRARY_LEN}"),
                Some(PrefabUnreadableSource::EntrySize),
            ));
        }
        let reader = self.archive.open_entry(entry).map_err(|error| {
            if error.is_key_required() || error.is_wrong_key() {
                Failure::Fatal(Box::new(error))
            } else {
                Failure::unreadable(error.to_string(), Some(PrefabUnreadableSource::Unavailable))
            }
        })?;
        let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or_default());
        reader
            .take(MAX_PREFAB_LIBRARY_LEN)
            .read_to_end(&mut bytes)
            .map_err(|error| {
                Failure::unreadable(error.to_string(), Some(PrefabUnreadableSource::Unavailable))
            })?;
        parse(&bytes).map_err(Failure::parsed)
    }
}

fn file_of(slot: usize, name: String, parsed: PositionedPrefabLibrary) -> PrefabFile {
    let library = parsed.library;
    let mut by_name = HashMap::with_capacity(library.prefabs.len());
    for (index, prefab) in library.prefabs.iter().enumerate() {
        by_name
            .entry(prefab.name.to_ascii_lowercase())
            .or_insert(index);
    }
    PrefabFile {
        slot,
        name,
        library,
        source: parsed.source,
        by_name,
    }
}
