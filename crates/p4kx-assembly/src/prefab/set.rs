// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;

use p4kx_archive::{Archive, Entry};
use p4kx_model::{
    PositionedPrefabLibrary, PrefabError as LibraryReadError, PrefabLibrary, PrefabLibrarySource,
    PrefabRef,
};
use rayon::prelude::*;

use super::{MAX_PREFAB_LIBRARY_LEN, PrefabError};

const FOLDER: &str = "data/prefabs/";
const EXTENSION: &str = ".xml";

const GUID_INDEX_THREADS: usize = 4;

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

pub(super) enum Lookup {
    Found(Arc<PrefabFile>, usize),
    Missing(Option<(String, String)>),
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
    failed: HashMap<usize, String>,
    guids: Option<GuidIndex>,
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
    Unreadable(String),
}

impl<'a> PrefabSet<'a> {
    /// Indexes the libraries of `archive`; none is read yet.
    #[must_use]
    pub fn new(archive: &'a Archive) -> Self {
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
        if let Some(reason) = self.failed.get(&slot) {
            return Err(PrefabError::Unreadable {
                library: self.shown(slot),
                reason: reason.clone(),
            });
        }
        match self.read(slot, PrefabLibrary::parse_positioned) {
            Ok(parsed) => {
                let file = Arc::new(file_of(slot, self.shown(slot), parsed));
                self.loaded.insert(slot, Arc::clone(&file));
                Ok(file)
            }
            Err(Failure::Fatal(error)) => Err(PrefabError::Archive(error)),
            Err(Failure::Unreadable(reason)) => {
                self.failed.insert(slot, reason.clone());
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
        parse: fn(&[u8]) -> Result<T, LibraryReadError>,
    ) -> Result<T, Failure> {
        let entry = self
            .slots
            .get(slot)
            .map(|found| found.entry)
            .ok_or_else(|| Failure::Unreadable("the Entry is gone".to_owned()))?;
        let size = entry.uncompressed_size();
        if size > MAX_PREFAB_LIBRARY_LEN {
            return Err(Failure::Unreadable(format!(
                "it holds {size} bytes, more than the limit of {MAX_PREFAB_LIBRARY_LEN}"
            )));
        }
        let reader = self.archive.open_entry(entry).map_err(|error| {
            if error.is_key_required() || error.is_wrong_key() {
                Failure::Fatal(Box::new(error))
            } else {
                Failure::Unreadable(error.to_string())
            }
        })?;
        let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or_default());
        reader
            .take(MAX_PREFAB_LIBRARY_LEN)
            .read_to_end(&mut bytes)
            .map_err(|error| Failure::Unreadable(error.to_string()))?;
        parse(&bytes).map_err(|error| Failure::Unreadable(error.to_string()))
    }

    fn by_name(&mut self, reference: &PrefabRef) -> Result<Lookup, PrefabError> {
        let mut unreadable = None;
        for (at, _) in reference.name.match_indices('.') {
            let Some((library, rest)) = reference.name.split_at_checked(at) else {
                continue;
            };
            let prefab = rest.trim_start_matches('.');
            let Some(slot) = self.slot_of(library) else {
                continue;
            };
            if prefab.is_empty() {
                continue;
            }
            match self.load(slot) {
                Ok(file) => {
                    if let Some(index) = file.find(prefab) {
                        return Ok(Lookup::Found(file, index));
                    }
                }
                Err(PrefabError::Unreadable { library, reason }) => {
                    unreadable.get_or_insert((library, reason));
                }
                Err(error) => return Err(error),
            }
        }
        Ok(Lookup::Missing(unreadable))
    }

    fn guid_index(&mut self) -> Result<&GuidIndex, PrefabError> {
        if self.guids.is_none() {
            let ids_of = |slot: usize| match self.loaded.get(&slot) {
                Some(file) => Ok(ids(file.library())),
                None if self.failed.contains_key(&slot) => Ok(Vec::new()),
                None => self
                    .read(slot, PrefabLibrary::parse)
                    .map(|library| ids(&library)),
            };
            let count = self.slots.len();
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(GUID_INDEX_THREADS)
                .build();
            let read: Vec<Result<Vec<String>, Failure>> = pool.map_or_else(
                |_| (0..count).map(ids_of).collect(),
                |pool| pool.install(|| (0..count).into_par_iter().map(ids_of).collect()),
            );
            let mut index: GuidIndex = HashMap::new();
            for (slot, result) in read.into_iter().enumerate() {
                match result {
                    Ok(found) => {
                        for (prefab, id) in found.into_iter().enumerate() {
                            if !id.is_empty() {
                                index.entry(id).or_default().push((slot, prefab));
                            }
                        }
                    }
                    Err(Failure::Fatal(error)) => return Err(PrefabError::Archive(error)),
                    Err(Failure::Unreadable(reason)) => {
                        self.failed.insert(slot, reason);
                    }
                }
            }
            self.guids = Some(index);
        }
        Ok(self.guids.get_or_insert_with(HashMap::new))
    }

    /// Finds the prefab an object instances: the one of its `PrefabName` when that has the
    /// object's `PrefabGUID` or the object names no GUID, else the one with the GUID, else the
    /// one of the name.
    pub(super) fn find_nested(&mut self, reference: &PrefabRef) -> Result<Lookup, PrefabError> {
        let named = self.by_name(reference)?;
        let guid = reference.guid.to_ascii_lowercase();
        if guid.is_empty() {
            return Ok(named);
        }
        if let Lookup::Found(file, index) = &named
            && file
                .library()
                .prefabs
                .get(*index)
                .is_some_and(|prefab| prefab.id.eq_ignore_ascii_case(&guid))
        {
            return Ok(named);
        }
        let candidates = self.guid_index()?.get(&guid).cloned().unwrap_or_default();
        let preferred = match &named {
            Lookup::Found(file, _) => candidates
                .iter()
                .copied()
                .find(|(slot, _)| *slot == file.slot()),
            Lookup::Missing(_) => None,
        };
        let others = candidates
            .into_iter()
            .filter(|candidate| Some(*candidate) != preferred);
        for (slot, index) in preferred.into_iter().chain(others) {
            if let Ok(file) = self.load(slot) {
                return Ok(Lookup::Found(file, index));
            }
        }
        Ok(named)
    }
}

fn ids(library: &PrefabLibrary) -> Vec<String> {
    library
        .prefabs
        .iter()
        .map(|prefab| prefab.id.to_ascii_lowercase())
        .collect()
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
