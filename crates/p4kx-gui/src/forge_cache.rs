// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use p4kx_extraction::DataForgeLoadError;
use p4kx_extraction::query::QuerySchema;

use crate::EntryIndex;
use crate::model::describe;
use source::take_turn;
use waiting::Waiting;

mod source;
mod waiting;

pub(crate) use source::{ForgeSource, ReadFailure};

/// The one DataForge cache of an open Archive, shared by the Record tree, the Record preview, the
/// Query tab and the Record export.
///
/// The first consumer that needs DataForge reads it; the others wait for that read instead of
/// reading it again. A read finished by a consumer that did not ask to stop is kept until the
/// Archive changes on disk. A consumer that reads with [`read_until`](Self::read_until) can stop
/// the read: it is then thrown away and the next consumer reads again, unless another consumer
/// already waits for it, in which case the read goes on and is kept.
/// [`clear`](Self::clear) never waits for a read.
#[derive(Debug, Default)]
pub(crate) struct ForgeCache {
    reading: Mutex<()>,
    waiting: AtomicUsize,
    source: Mutex<Option<Arc<ForgeSource>>>,
    generation: AtomicU64,
    reads: AtomicUsize,
    schema_asked: AtomicU64,
    schema_attempted: AtomicU64,
}

impl ForgeCache {
    /// Returns whether DataForge was read; `false` while it is being read.
    pub(crate) fn is_read(&self) -> bool {
        self.source
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    }

    /// Returns how often DataForge was read from the Archive, whether or not the read succeeded.
    pub(crate) fn reads(&self) -> usize {
        self.reads.load(Ordering::Relaxed)
    }

    /// Returns the DataForge of `index`, reading it unless it was read before; `reading` is
    /// called before a read starts.
    ///
    /// # Errors
    ///
    /// Returns why DataForge cannot be read, or that the Archive changed on disk before or while
    /// it was read.
    pub(crate) fn read(
        &self,
        index: &EntryIndex,
        reading: impl FnOnce(),
    ) -> Result<Arc<ForgeSource>, ReadFailure> {
        let waiting = Waiting::new(&self.waiting);
        let _turn = self.reading.lock().unwrap_or_else(PoisonError::into_inner);
        drop(waiting);
        self.read_locked(index, reading, &|| false)?
            .ok_or_else(|| ReadFailure::Unreadable(describe(&DataForgeLoadError::Cancelled)))
    }

    /// Returns the DataForge of `index` like [`read`](Self::read), or `None` once `stop` returns
    /// `true`, while waiting for the read of another consumer or while reading. A read that was
    /// stopped leaves nothing in the cache; one that another consumer waits for is not stopped.
    ///
    /// # Errors
    ///
    /// Returns why DataForge cannot be read, or that the Archive changed on disk before or while
    /// it was read.
    pub(crate) fn read_until(
        &self,
        index: &EntryIndex,
        reading: impl FnOnce(),
        stop: &dyn Fn() -> bool,
    ) -> Result<Option<Arc<ForgeSource>>, ReadFailure> {
        let waiting = Waiting::new(&self.waiting);
        let Some(_turn) = take_turn(&self.reading, stop) else {
            return Ok(None);
        };
        drop(waiting);
        let stop = || stop() && self.waiting.load(Ordering::Acquire) == 0;
        self.read_locked(index, reading, &stop)
    }

    fn read_locked(
        &self,
        index: &EntryIndex,
        reading: impl FnOnce(),
        stop: &dyn Fn() -> bool,
    ) -> Result<Option<Arc<ForgeSource>>, ReadFailure> {
        if let Some(source) = self.cached() {
            return Ok(Some(source));
        }
        self.reads.fetch_add(1, Ordering::Relaxed);
        let generation = self.generation.load(Ordering::Acquire);
        let changed = |error| ReadFailure::ArchiveChanged(describe(&error));
        index.watch().check().map_err(changed)?;
        reading();
        let entry = match index.forge_source().read_until(stop) {
            Ok(entry) => entry,
            Err(DataForgeLoadError::Cancelled) => return Ok(None),
            Err(error) => return Err(ReadFailure::Unreadable(describe(&error))),
        };
        index.watch().check().map_err(changed)?;
        let read = Arc::new(ForgeSource::new(entry));
        if self.store(generation, &read) {
            Ok(Some(read))
        } else {
            Err(ReadFailure::ArchiveChanged(
                "the DataForge cache was cleared while it was read".to_owned(),
            ))
        }
    }

    /// Returns the completion schema once it was built from the DataForge held now.
    pub(crate) fn schema(&self) -> Option<Arc<QuerySchema>> {
        self.cached()?.schema_if_built()
    }

    /// Returns how often the index of References was built for the DataForge held now.
    pub(crate) fn reference_builds(&self) -> usize {
        self.cached().map_or(0, |source| source.references_builds())
    }

    /// Returns how often the tag tree was built for the DataForge held now.
    pub(crate) fn tag_builds(&self) -> usize {
        self.cached().map_or(0, |source| source.tags_builds())
    }

    /// Returns how often the completion schema was built for the DataForge held now.
    pub(crate) fn schema_builds(&self) -> usize {
        self.cached().map_or(0, |source| source.schema_builds())
    }

    /// Returns whether the completion schema is missing, no build runs and none was started
    /// since the cache was last cleared.
    pub(crate) fn schema_unattempted(&self) -> bool {
        let current = self.generation.load(Ordering::Acquire).wrapping_add(1);
        self.schema_asked.load(Ordering::Acquire) != current
            && self.schema_attempted.load(Ordering::Acquire) != current
            && self.schema().is_none()
    }

    /// Builds the completion schema of `index` on a background thread, reading its DataForge
    /// unless it was read before, and calls `wake` once it is there. Nothing starts while a build
    /// of the current DataForge runs or after one succeeded; after a failed build or spawn the
    /// next call tries again. A build still running for a DataForge that was cleared meanwhile
    /// does not hold the call back, and its schema is dropped with that DataForge.
    pub(crate) fn prepare_schema(index: &Arc<EntryIndex>, wake: impl Fn() + Send + 'static) {
        let cache = index.forge();
        let current = cache.generation.load(Ordering::Acquire).wrapping_add(1);
        if cache.schema().is_some() || cache.schema_asked.swap(current, Ordering::AcqRel) == current
        {
            return;
        }
        cache.schema_attempted.store(current, Ordering::Release);
        let thread_index = Arc::clone(index);
        let spawned = thread::Builder::new()
            .name("query-schema".to_owned())
            .spawn(move || {
                let cache = thread_index.forge();
                let built = cache
                    .read(&thread_index, || {})
                    .and_then(|source| source.schema());
                let _ = cache.schema_asked.compare_exchange(
                    current,
                    0,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
                if built.is_ok() {
                    wake();
                }
            });
        if spawned.is_err() {
            let _ = cache.schema_asked.compare_exchange(
                current,
                0,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }

    /// Forgets the DataForge read, because the Archive changed on disk, and makes a read still
    /// running discard its result; consumers that hold the DataForge keep their copy until they
    /// end. Returns at once, whatever reads.
    pub(crate) fn clear(&self) {
        let mut source = self.source.lock().unwrap_or_else(PoisonError::into_inner);
        self.generation.fetch_add(1, Ordering::AcqRel);
        *source = None;
    }

    fn store(&self, generation: u64, read: &Arc<ForgeSource>) -> bool {
        let mut source = self.source.lock().unwrap_or_else(PoisonError::into_inner);
        let current = self.generation.load(Ordering::Acquire) == generation;
        if current {
            *source = Some(Arc::clone(read));
        }
        current
    }

    fn cached(&self) -> Option<Arc<ForgeSource>> {
        self.source
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[cfg(test)]
mod tags_tests;
#[cfg(test)]
pub(crate) mod tests;
