// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, TryLockError};
use std::thread;
use std::time::Duration;

use p4kx_export::RecordFilePlans;
use p4kx_extraction::TagTree;
use p4kx_extraction::query::QuerySchema;
use p4kx_extraction::{DataForgeEntry, ReferenceIndex};

use crate::ModelExportFailure;
use crate::model::describe;

/// How often a consumer waiting for its turn to read DataForge or to build an index looks at its
/// stop flag.
const TURN_POLL: Duration = Duration::from_millis(25);

/// Takes `turn`, waiting for the consumer that holds it; returns `None` once `stop` returns `true`
/// while waiting.
pub(super) fn take_turn<'a>(
    turn: &'a Mutex<()>,
    stop: &dyn Fn() -> bool,
) -> Option<MutexGuard<'a, ()>> {
    loop {
        match turn.try_lock() {
            Ok(guard) => return Some(guard),
            Err(TryLockError::Poisoned(poisoned)) => return Some(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => {
                if stop() {
                    return None;
                }
                thread::sleep(TURN_POLL);
            }
        }
    }
}

/// The DataForge of one open Archive as every consumer reads it: its Entry, read once, the files
/// of its Records, planned once per form, and the Query completion schema, built once.
pub(crate) struct ForgeSource {
    pub(crate) entry: Arc<DataForgeEntry>,
    pub(crate) plans: RecordFilePlans,
    schema: OnceLock<Arc<QuerySchema>>,
    schema_turn: Mutex<()>,
    schema_builds: AtomicUsize,
    references: OnceLock<Arc<ReferenceIndex>>,
    references_turn: Mutex<()>,
    references_builds: AtomicUsize,
    tags: OnceLock<Arc<TagTree>>,
    tags_turn: Mutex<()>,
    tags_builds: AtomicUsize,
}

impl ForgeSource {
    pub(super) fn new(entry: DataForgeEntry) -> Self {
        Self {
            entry: Arc::new(entry),
            plans: RecordFilePlans::default(),
            schema: OnceLock::new(),
            schema_turn: Mutex::new(()),
            schema_builds: AtomicUsize::new(0),
            references: OnceLock::new(),
            references_turn: Mutex::new(()),
            references_builds: AtomicUsize::new(0),
            tags: OnceLock::new(),
            tags_turn: Mutex::new(()),
            tags_builds: AtomicUsize::new(0),
        }
    }

    /// Returns the completion schema if it was built.
    pub(crate) fn schema_if_built(&self) -> Option<Arc<QuerySchema>> {
        self.schema.get().cloned()
    }

    /// Returns how often the completion schema was built.
    pub(super) fn schema_builds(&self) -> usize {
        self.schema_builds.load(Ordering::Relaxed)
    }

    /// Returns how often the index of References was built.
    pub(super) fn references_builds(&self) -> usize {
        self.references_builds.load(Ordering::Relaxed)
    }

    /// Returns how often the tag tree was built.
    pub(super) fn tags_builds(&self) -> usize {
        self.tags_builds.load(Ordering::Relaxed)
    }

    /// Returns the completion schema, building it from the DataForge already read unless it was
    /// built before. The build runs before publishing the immutable schema.
    ///
    /// # Errors
    ///
    /// Returns why the DataForge cannot be opened or its completion schema cannot be built.
    pub(crate) fn schema(&self) -> Result<Arc<QuerySchema>, ReadFailure> {
        let _turn = self
            .schema_turn
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(schema) = self.schema.get() {
            return Ok(Arc::clone(schema));
        }
        let forge = self
            .entry
            .open()
            .map_err(|error| ReadFailure::Unreadable(describe(&error)))?;
        self.schema_builds.fetch_add(1, Ordering::Relaxed);
        let built = Arc::new(
            QuerySchema::of(&forge).map_err(|error| ReadFailure::Unreadable(describe(&error)))?,
        );
        Ok(Arc::clone(self.schema.get_or_init(|| built)))
    }

    /// Returns the tag tree of the DataForge already read, building it unless it was built
    /// before, or `None` once `stop` returns `true`, while building or while waiting for the build
    /// of another consumer. The build runs outside the [`OnceLock`], and one at a time, so that it
    /// happens once per read.
    ///
    /// # Errors
    ///
    /// Returns why the DataForge cannot be opened.
    pub(crate) fn tags(
        &self,
        stop: &dyn Fn() -> bool,
    ) -> Result<Option<Arc<TagTree>>, ReadFailure> {
        if let Some(tags) = self.tags.get() {
            return Ok(Some(Arc::clone(tags)));
        }
        let Some(_turn) = take_turn(&self.tags_turn, stop) else {
            return Ok(None);
        };
        if let Some(tags) = self.tags.get() {
            return Ok(Some(Arc::clone(tags)));
        }
        let forge = self
            .entry
            .open()
            .map_err(|error| ReadFailure::Unreadable(describe(&error)))?;
        let Some(built) = TagTree::build_with(&forge, stop) else {
            return Ok(None);
        };
        self.tags_builds.fetch_add(1, Ordering::Relaxed);
        let built = Arc::new(built);
        Ok(Some(Arc::clone(self.tags.get_or_init(|| built))))
    }

    /// Returns the index of the References between the Records, building it from the DataForge
    /// already read unless it was built before, or `None` once `stop` returns `true`, while
    /// building or while waiting for the build of another consumer. The build runs outside the
    /// [`OnceLock`], because it uses rayon, and one at a time, so that it happens once per read.
    ///
    /// # Errors
    ///
    /// Returns why the DataForge cannot be opened.
    pub(crate) fn references(
        &self,
        stop: &(dyn Fn() -> bool + Sync),
    ) -> Result<Option<Arc<ReferenceIndex>>, ReadFailure> {
        if let Some(references) = self.references.get() {
            return Ok(Some(Arc::clone(references)));
        }
        let Some(_turn) = take_turn(&self.references_turn, stop) else {
            return Ok(None);
        };
        if let Some(references) = self.references.get() {
            return Ok(Some(Arc::clone(references)));
        }
        let forge = self
            .entry
            .open()
            .map_err(|error| ReadFailure::Unreadable(describe(&error)))?;
        let Some(built) = ReferenceIndex::build_cancellable(&forge, stop) else {
            return Ok(None);
        };
        self.references_builds.fetch_add(1, Ordering::Relaxed);
        let built = Arc::new(built);
        Ok(Some(Arc::clone(self.references.get_or_init(|| built))))
    }
}

impl fmt::Debug for ForgeSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ForgeSource")
            .field("entry", &self.entry.path())
            .field("plans", &self.plans)
            .field("schema", &self.schema.get().is_some())
            .field("schema_builds", &self.schema_builds)
            .field("references", &self.references.get().is_some())
            .field("references_builds", &self.references_builds)
            .field("tags", &self.tags.get().is_some())
            .field("tags_builds", &self.tags_builds)
            .finish_non_exhaustive()
    }
}

/// Why the DataForge of an Archive could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReadFailure {
    /// The Archive changed on disk before or while it was read, for the reason given.
    ArchiveChanged(String),
    /// The Entry cannot be read, for the reason given.
    Unreadable(String),
}

impl ReadFailure {
    /// Returns the reason.
    pub(crate) fn into_reason(self) -> String {
        match self {
            Self::ArchiveChanged(reason) | Self::Unreadable(reason) => reason,
        }
    }

    /// Returns the failure of an export that could not read the DataForge.
    pub(crate) fn into_export(self) -> ModelExportFailure {
        match self {
            Self::ArchiveChanged(_) => ModelExportFailure::ArchiveChanged,
            Self::Unreadable(reason) => ModelExportFailure::Failed(reason),
        }
    }
}
