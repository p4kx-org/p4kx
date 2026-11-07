// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_dataforge::{DataForgeError, Guid, Record, RecordTree, TreeBuilder};

use crate::{
    Entities, LoadoutDocumentKind, LoadoutError, LoadoutFileError, LoadoutFileIdentity,
    LoadoutResolver,
};

/// An actual source operation performed by a loadout resolver.
#[derive(Debug)]
pub enum LoadoutReadEvent<'r, 'f> {
    /// A traversal limit prevents enumerating further required source inputs.
    InventoryIncomplete,
    /// An actual required XML file read completed.
    FileRead {
        /// The actual Entry identity or configured missing target.
        source: LoadoutFileIdentity<'r>,
        /// The intrinsic document projection requested by the reader.
        kind: LoadoutDocumentKind,
        /// The actual byte-read outcome.
        outcome: Result<(), &'r LoadoutFileError>,
    },
    /// The actual intrinsic source document projection completed.
    FileProjection {
        /// The same source identity as the preceding successful file read.
        source: LoadoutFileIdentity<'r>,
        /// The intrinsic document projection performed.
        kind: LoadoutDocumentKind,
        /// The actual parse or document-shape outcome.
        outcome: Result<(), &'r str>,
    },
    /// A canonical dependency read completed before optional projection helpers run.
    CanonicalRead {
        /// The physical dependency Record.
        record: &'r Record<'f>,
        /// The actual canonical result.
        outcome: Result<&'r RecordTree<'f>, &'r DataForgeError>,
    },
    /// A caller supplied the canonical root tree instead of decoding it again.
    ProvidedTree {
        /// The physical source identity supplied by the caller.
        record: &'r Record<'f>,
        /// The supplied immutable canonical tree.
        tree: &'r RecordTree<'f>,
    },
    /// A previously read physical source supplies a cached projection.
    Reused {
        /// The original physical source behind the cache entry.
        record: &'r Record<'f>,
    },
    /// A known dependency cannot supply the required source type before canonical reading.
    Rejected {
        /// The physical dependency Record.
        record: &'r Record<'f>,
        /// The typed source-selection error.
        error: &'r LoadoutError,
    },
}

/// Observes actual resolver source operations, including concurrent prefetch reads.
pub trait LoadoutReadObserver: Send + Sync {
    /// Returns `Break` to stop the observed resolution.
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()>;
}

pub(crate) struct Observation {
    observer: Arc<dyn LoadoutReadObserver>,
    stopped: AtomicBool,
}

impl Observation {
    fn new(observer: Arc<dyn LoadoutReadObserver>) -> Self {
        Self {
            observer,
            stopped: AtomicBool::new(false),
        }
    }

    pub(crate) fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }

    fn notify(&self, event: LoadoutReadEvent<'_, '_>) -> Result<(), LoadoutError> {
        if self.observer.observe(event).is_break() {
            self.stopped.store(true, Ordering::Release);
        }
        if self.stopped() {
            Err(LoadoutError::ObservationStopped)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Cache {
    Assortment,
    LandingGear,
}

pub(crate) fn notify(
    observation: Option<&Observation>,
    event: LoadoutReadEvent<'_, '_>,
) -> Result<(), LoadoutError> {
    observation.map_or(Ok(()), |observation| observation.notify(event))
}

pub(crate) fn canonical<'f>(
    builder: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    observation: Option<&Observation>,
) -> Result<RecordTree<'f>, LoadoutError> {
    if observation.is_some_and(Observation::stopped) {
        return Err(LoadoutError::ObservationStopped);
    }
    let outcome = builder.build(record);
    notify(
        observation,
        LoadoutReadEvent::CanonicalRead {
            record,
            outcome: outcome.as_ref(),
        },
    )?;
    outcome.map_err(|error| LoadoutError::UnreadableRecord {
        guid: record.guid(),
        reason: error.to_string(),
    })
}

impl<'f> LoadoutResolver<'f> {
    /// Creates a fresh observed resolver without claiming evidence for previously cached sources.
    #[must_use]
    pub fn with_observed_entities(
        entities: Arc<Entities<'f>>,
        observer: Arc<dyn LoadoutReadObserver>,
    ) -> Self {
        let mut resolver = Self::with_entities(entities);
        resolver.observation = Some(Arc::new(Observation::new(observer)));
        resolver
    }

    pub(crate) fn observation_stopped(&self) -> bool {
        self.observation
            .as_deref()
            .is_some_and(Observation::stopped)
    }

    pub(crate) fn remember_read(&mut self, cache: Cache, record: Record<'f>) {
        if self.observation.is_some() {
            self.read_sources.insert((cache, record.guid()), record);
        }
    }

    pub(crate) fn reuse_read(&self, cache: Cache, guid: Guid) -> Result<(), LoadoutError> {
        if let Some(record) = self.read_sources.get(&(cache, guid)) {
            notify(
                self.observation.as_deref(),
                LoadoutReadEvent::Reused { record },
            )?;
        }
        Ok(())
    }
}

pub(crate) fn read_spec<'f>(
    builder: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    observation: Option<&Observation>,
) -> Result<crate::spec::EntitySpec, LoadoutError> {
    if !crate::entities::is_entity(record) {
        let error = LoadoutError::NotAnEntity {
            guid: record.guid(),
            type_name: record
                .type_name()
                .unwrap_or("Record of an unreadable type")
                .to_owned(),
        };
        notify(
            observation,
            LoadoutReadEvent::Rejected {
                record,
                error: &error,
            },
        )?;
        return Err(error);
    }
    canonical(builder, record, observation).map(|tree| crate::spec::EntitySpec::read(&tree))
}

impl LoadoutResolver<'_> {
    /// Returns how many Records were read one at a time while a tree was built, outside the
    /// parallel prefetch.
    #[doc(hidden)]
    #[must_use]
    pub const fn serial_reads(&self) -> usize {
        self.serial_reads
    }
}

#[cfg(test)]
#[path = "resolver_observation_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "resolver_alias_fixture.rs"]
pub(crate) mod alias_fixture;
