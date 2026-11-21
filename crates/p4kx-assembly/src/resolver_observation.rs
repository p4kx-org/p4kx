// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_dataforge::{Guid, Record, RecordTree, TreeBuilder};

pub(crate) use crate::resolver_path_read::parse_spec;

use crate::{
    Entities, LoadoutClassSelection, LoadoutError, LoadoutFileObservation, LoadoutPath,
    LoadoutPathObserver, LoadoutRequest, LoadoutResolver, LoadoutSelection, LoadoutTargetGuid,
    LoadoutXmlRecordSelector, RecordLoadoutEffect, VehicleImplementation, XmlLoadoutClassSite,
    XmlLoadoutPathSite, XmlPortReplacement,
};

#[path = "loadout_read_event.rs"]
mod loadout_read_event;
pub use loadout_read_event::LoadoutReadEvent;

#[path = "loadout_read_observer.rs"]
mod loadout_read_observer;

pub use loadout_read_observer::LoadoutReadObserver;

struct VehiclePortAttributeCapture;

impl LoadoutReadObserver for VehiclePortAttributeCapture {
    fn observe(&self, _event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn observes_vehicle_port_attributes(&self) -> bool {
        true
    }
}

pub(crate) struct Observation<'f> {
    observer: Arc<dyn LoadoutReadObserver + 'f>,
    stopped: AtomicBool,
}

impl<'f> Observation<'f> {
    pub(crate) fn new(observer: Arc<dyn LoadoutReadObserver + 'f>) -> Self {
        Self {
            observer,
            stopped: AtomicBool::new(false),
        }
    }

    pub(crate) fn capture_vehicle_port_attributes() -> Self {
        Self::new(Arc::new(VehiclePortAttributeCapture))
    }

    pub(crate) fn observes_record_loadout_effects(&self) -> bool {
        self.observer.observes_record_loadout_effects()
    }

    pub(crate) fn record_loadout_effect(&self, effect: &RecordLoadoutEffect<'_>) {
        if !self.stopped() && self.observer.record_loadout_effect(effect).is_break() {
            self.stop();
        }
    }

    pub(crate) fn observes_port_names(&self) -> bool {
        self.observer.observes_port_names()
    }

    pub(crate) fn observes_xml_port_replacements(&self) -> bool {
        self.observer.observes_xml_port_replacements()
    }

    pub(crate) fn xml_port_replacements_used(
        &self,
        original: &LoadoutFileObservation,
        request: &LoadoutPath,
        replacements: &[XmlPortReplacement],
    ) -> Result<(), LoadoutError> {
        if !self.stopped()
            && self
                .observer
                .xml_port_replacements_used(original, request, replacements)
                .is_break()
        {
            self.stop();
        }
        if self.stopped() {
            Err(LoadoutError::ObservationStopped)
        } else {
            Ok(())
        }
    }

    pub(crate) fn observes_xml_record_selectors(&self) -> bool {
        self.observer.observes_xml_record_selectors()
    }

    pub(crate) fn xml_record_selector(
        &self,
        site: &LoadoutXmlRecordSelector<'_, '_>,
    ) -> ControlFlow<()> {
        if self.stopped() {
            return ControlFlow::Break(());
        }
        self.observer.xml_record_selector(site)
    }

    pub(crate) fn observes_vehicle_visibility(&self) -> bool {
        self.observer.observes_vehicle_visibility()
    }

    pub(crate) fn observes_vehicle_port_attributes(&self) -> bool {
        self.observer.observes_vehicle_port_attributes()
    }

    pub(crate) fn observes_vehicle_totals(&self) -> bool {
        self.observer.observes_vehicle_totals()
    }

    pub(crate) fn vehicle_used(
        &self,
        record_index: u32,
        implementation: &VehicleImplementation,
        caller: &LoadoutRequest,
    ) {
        if !self.stopped()
            && self
                .observer
                .vehicle_used(record_index, implementation, caller)
                .is_break()
        {
            self.stop();
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
    forge_origin: Option<&Arc<p4kx_extraction::DataForgeOrigin>>,
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
    parse_spec(record, &tree, observation, forge_origin)
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
