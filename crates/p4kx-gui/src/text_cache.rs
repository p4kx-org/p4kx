// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use p4kx_extraction::{
    IndexedRecordLocalization, LocalizationEntry, LocalizationLoadError, Resolution, TextResolver,
    is_placeholder_key,
};

use crate::model::describe;
use crate::{EntryIndex, TextLanguage};

/// Why the texts of a language could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TextFailure {
    /// The Archive changed on disk before or while the texts were read.
    ArchiveChanged,
    /// The `global.ini` cannot be read or indexed, for the reason given.
    Unreadable(String),
}

#[derive(Debug)]
pub(crate) struct CachedLanguage {
    pub(crate) indexed: Option<IndexedRecordLocalization>,
}

impl TextResolver for CachedLanguage {
    fn resolve<'resolver>(&'resolver self, key: &str) -> Resolution<'resolver> {
        match &self.indexed {
            Some(locale) => locale.resolve(key),
            None if is_placeholder_key(key) => Resolution::Keep,
            None => Resolution::Missing,
        }
    }
}

type Loaded = Result<Arc<CachedLanguage>, TextFailure>;

/// The localisation texts of an open Archive, one language at a time.
///
/// The first consumer loads the language Entry and builds its positional index.
/// Other consumers wait for that load. A failed load is kept as well, so a broken `global.ini` is not
/// read again for every Record. [`clear`](Self::clear) forgets everything once the Archive
/// changed on disk.
#[derive(Debug, Default)]
pub(crate) struct TextCache {
    slots: Mutex<Vec<(TextLanguage, Arc<OnceLock<Loaded>>)>>,
    loads: AtomicUsize,
}

impl TextCache {
    /// Returns how often a `global.ini` was read from the Archive, whether or not it succeeded.
    pub(crate) fn loads(&self) -> usize {
        self.loads.load(Ordering::Relaxed)
    }

    /// Returns the texts of `language` in `index`, loading them unless they were loaded before.
    ///
    /// # Errors
    ///
    /// Returns why the `global.ini` of the language cannot be loaded.
    pub(crate) fn load(&self, index: &EntryIndex, language: TextLanguage) -> Loaded {
        let slot = self.slot(language);
        slot.get_or_init(|| self.read(index, language)).clone()
    }

    /// Forgets every loaded language, because the Archive changed on disk; consumers that hold
    /// texts keep their copy until they end.
    pub(crate) fn clear(&self) {
        self.slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }

    fn slot(&self, language: TextLanguage) -> Arc<OnceLock<Loaded>> {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, slot)) = slots.iter().find(|(known, _)| *known == language) {
            return Arc::clone(slot);
        }
        let slot = Arc::new(OnceLock::new());
        slots.push((language, Arc::clone(&slot)));
        slot
    }

    fn read(&self, index: &EntryIndex, language: TextLanguage) -> Loaded {
        self.loads.fetch_add(1, Ordering::Relaxed);
        let changed = |_| TextFailure::ArchiveChanged;
        index.watch().check().map_err(changed)?;
        let unreadable = |error: &dyn std::error::Error| TextFailure::Unreadable(describe(error));
        let archive = index
            .entries_archive()
            .map_err(|error| unreadable(&error))?;
        let indexed = match LocalizationEntry::read(archive, language) {
            Ok(entry) => Some(
                IndexedRecordLocalization::try_new(entry).map_err(|error| unreadable(&error))?,
            ),
            Err(LocalizationLoadError::Missing { .. }) => None,
            Err(error) => return Err(unreadable(&error)),
        };
        index.watch().check().map_err(changed)?;
        Ok(Arc::new(CachedLanguage { indexed }))
    }
}
