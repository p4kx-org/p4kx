// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_dataforge::{DataForgeError, Guid, Record, RecordTree, TreeBuilder};

pub(crate) use crate::resolver_path_read::parse_spec;

use crate::{
    Entities, LoadoutClassSelection, LoadoutError, LoadoutFileObservation, LoadoutPath,
    LoadoutPathObserver, LoadoutRequest, LoadoutResolver, LoadoutSelection, LoadoutTargetGuid,
    XmlLoadoutClassSite, XmlLoadoutPathSite,
};

/// An actual source operation performed by a loadout resolver.
#[derive(Debug)]
pub enum LoadoutReadEvent<'r, 'f> {
    /// A traversal limit prevents enumerating further required source inputs.
    InventoryIncomplete,
    /// An actual required XML file read completed.
    FileRead {
        /// The original document operation whose read completed.
        observation: &'r LoadoutFileObservation,
        /// The actual retained document candidate that supplied this operation.
        request: &'r LoadoutPath,
        /// The current caller supplied before document-cache lookup.
        caller: &'r LoadoutRequest,
    },
    /// The actual intrinsic source document projection completed.
    FileProjection {
        /// The same original operation whose successful read was observed.
        observation: &'r LoadoutFileObservation,
        /// The actual retained document candidate that supplied this operation.
        request: &'r LoadoutPath,
        /// The current caller supplied before document-cache lookup.
        caller: &'r LoadoutRequest,
    },
    /// Original file and projection outcomes supplied a cached document result.
    FileReused {
        /// Immutable first-population evidence, without another source operation.
        observation: &'r LoadoutFileObservation,
        /// The actual retained document candidate that supplied this operation.
        request: &'r LoadoutPath,

        /// The current caller supplied before document-cache lookup.
        caller: &'r LoadoutRequest,
    },
    /// One original direct Include links two genuine file operations.
    FileDependency {
        /// The original including document's operation.
        parent: &'r LoadoutFileObservation,
        /// The original child operation, including an actual read or parse failure.
        observation: &'r LoadoutFileObservation,
        /// The child's original first-population candidate.
        request: &'r LoadoutPath,
        /// The genuine direct Include request selected by the parent parse.
        dependency: &'r LoadoutRequest,
    },
    /// A genuine Include is refused before a child operation is read.
    FileDependencyRefused {
        /// The original including document's operation.
        parent: &'r LoadoutFileObservation,
        /// The genuine Include request that reached this refusal.
        dependency: &'r LoadoutRequest,
        /// The actual cycle or input-limit failure.
        error: &'r LoadoutError,
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

    /// Runs the actual Record projection exactly once within a borrowed path scope.
    fn with_record_paths(
        &self,
        _record: &Record<'_>,
        _tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        parse(None)
    }

    /// Captures an actual same-parse XML attribute before its path text is copied.
    fn xml_path(
        &self,
        _selection: &LoadoutSelection,
        _site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    /// Captures the actual XML class-name attribute before its text is copied.
    fn xml_class(
        &self,
        _selection: &LoadoutClassSelection,
        _site: &XmlLoadoutClassSite<'_>,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    /// Captures the actual Record chosen for this class getter before copying its GUID.
    fn class_record(
        &self,
        _selection: &LoadoutClassSelection,
        _record: &Record<'_>,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    /// Captures one concrete candidate builder from its actual private input pairs.
    fn derived_path(
        &self,
        _selection: &LoadoutSelection,
        _inputs: &[&LoadoutPath],
        _value: &str,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }
}

pub(crate) struct Observation<'f> {
    observer: Arc<dyn LoadoutReadObserver + 'f>,
    stopped: AtomicBool,
}

impl<'f> Observation<'f> {
    fn new(observer: Arc<dyn LoadoutReadObserver + 'f>) -> Self {
        Self {
            observer,
            stopped: AtomicBool::new(false),
        }
    }

    pub(crate) fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }

    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
    }

    pub(crate) fn record_paths(
        &self,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        if self.stopped() {
            return ControlFlow::Break(());
        }
        self.observer.with_record_paths(record, tree, parse)
    }

    pub(crate) fn xml_path(
        &self,
        selection: &LoadoutSelection,
        site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        if self.stopped() {
            return ControlFlow::Break(());
        }
        let outcome = self.observer.xml_path(selection, site);
        if outcome.is_break() {
            self.stop();
        }
        outcome
    }

    pub(crate) fn xml_class(
        &self,
        selection: &LoadoutClassSelection,
        site: &XmlLoadoutClassSite<'_>,
    ) -> ControlFlow<()> {
        if self.stopped() {
            return ControlFlow::Break(());
        }
        let outcome = self.observer.xml_class(selection, site);
        if outcome.is_break() {
            self.stop();
        }
        outcome
    }

    pub(crate) fn class_record(
        &self,
        selection: &LoadoutClassSelection,
        record: &Record<'_>,
    ) -> Result<(), LoadoutError> {
        if self.stopped() {
            return Err(LoadoutError::ObservationStopped);
        }
        if self.observer.class_record(selection, record).is_break() {
            self.stop();
        }
        if self.stopped() {
            Err(LoadoutError::ObservationStopped)
        } else {
            Ok(())
        }
    }

    pub(crate) fn derived_path(
        &self,
        selection: &LoadoutSelection,
        inputs: &[&LoadoutPath],
        value: &str,
    ) -> Result<(), LoadoutError> {
        if self.stopped() {
            return Err(LoadoutError::ObservationStopped);
        }
        if self
            .observer
            .derived_path(selection, inputs, value)
            .is_break()
        {
            self.stop();
            return Err(LoadoutError::ObservationStopped);
        }
        Ok(())
    }
    fn notify(&self, event: LoadoutReadEvent<'_, '_>) -> Result<(), LoadoutError> {
        if self.stopped() {
            return Err(LoadoutError::ObservationStopped);
        }
        if self.observer.observe(event).is_break() {
            self.stop();
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
    observation: Option<&Observation<'_>>,
    event: LoadoutReadEvent<'_, '_>,
) -> Result<(), LoadoutError> {
    observation.map_or(Ok(()), |observation| observation.notify(event))
}

pub(crate) fn canonical<'f>(
    builder: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    observation: Option<&Observation<'_>>,
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
        guid: LoadoutTargetGuid::unbound(record.guid()),
        reason: error.to_string(),
    })
}

impl<'f> LoadoutResolver<'f> {
    /// Creates a fresh observed resolver without claiming evidence for previously cached sources.
    #[must_use]
    pub fn with_observed_entities(
        entities: Arc<Entities<'f>>,
        observer: Arc<dyn LoadoutReadObserver + 'f>,
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
    observation: Option<&Observation<'_>>,
) -> Result<crate::spec::EntitySpec, LoadoutError> {
    if observation.is_some_and(Observation::stopped) {
        return Err(LoadoutError::ObservationStopped);
    }
    if !crate::entities::is_entity(record) {
        let error = LoadoutError::NotAnEntity {
            guid: LoadoutTargetGuid::unbound(record.guid()),
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
    let tree = canonical(builder, record, observation)?;
    parse_spec(record, &tree, observation)
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
