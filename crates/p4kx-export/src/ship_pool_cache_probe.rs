// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_assembly::{
    LoadoutDocumentKind, LoadoutFileError, LoadoutFileObservation, LoadoutFileSource, LoadoutPath,
    LoadoutPathObserver, LoadoutReadEvent, LoadoutReadObserver, LoadoutSelection,
    XmlLoadoutPathSite,
};
use p4kx_extraction::{Archive, CanonicalReadObserver, DataForgeError, Record, RecordTree};

use super::fixture::{self, Case};
use crate::build::original_input::test_support::TestResult;
use crate::loadout::ledger::Capture;

struct Read {
    source: LoadoutFileSource,
    kind: LoadoutDocumentKind,
    outcome: Result<(), LoadoutFileError>,
}

struct Projection {
    source: LoadoutFileSource,
    kind: LoadoutDocumentKind,
    outcome: Result<(), String>,
}

#[derive(Default)]
struct Events {
    reads: Vec<Read>,
    projections: Vec<Projection>,
    reused: Vec<LoadoutFileObservation>,
}

/// Records genuine callbacks and forwards every event to the actual production Capture.
pub(super) struct Probe<'input> {
    pub(super) capture: Arc<Capture<'input>>,
    events: Mutex<Events>,
}

impl<'input> Probe<'input> {
    pub(super) fn new(capture: Arc<Capture<'input>>) -> Self {
        Self {
            capture,
            events: Mutex::new(Events::default()),
        }
    }

    pub(super) fn first(&self, archive: &Archive, case: Case) -> TestResult {
        let events = self
            .events
            .lock()
            .map_err(|_| "the source witness is poisoned")?;
        assert_eq!(
            events.reads.len(),
            1,
            "one actual resolver byte-read callback"
        );
        assert_eq!(
            events.projections.len(),
            usize::from(case.projected().is_some()),
            "the original read determines whether an intrinsic projection occurred"
        );
        assert!(
            events.reused.is_empty(),
            "the first population is not invented reuse"
        );
        let read = events.reads.first().ok_or("actual first file read")?;
        admission(&read.source, archive, case)?;
        original_outcomes(&events, read, archive, case)?;
        drop(events);
        Ok(())
    }

    pub(super) fn second(&self, archive: &Archive, case: Case) -> TestResult {
        let events = self
            .events
            .lock()
            .map_err(|_| "the source witness is poisoned")?;
        assert_eq!(
            events.reads.len(),
            1,
            "reuse performs no second resolver byte read"
        );
        assert_eq!(
            events.projections.len(),
            usize::from(case.projected().is_some()),
            "reuse performs no second intrinsic parse"
        );
        assert_eq!(
            events.reused.len(),
            1,
            "the second genuine caller reaches cached evidence"
        );
        let read = events.reads.first().ok_or("retained actual read")?;
        let cached = events
            .reused
            .first()
            .ok_or("actual immutable cached receipt")?;
        admission(cached.source(), archive, case)?;
        assert_eq!(cached.kind(), read.kind, "the real cached intrinsic kind");
        assert_eq!(
            cached.read_outcome().map_err(Clone::clone),
            read.outcome,
            "the cached receipt retains the exact original typed read result"
        );
        let original = events
            .projections
            .first()
            .map(|projection| projection.outcome.clone());
        assert_eq!(
            cached
                .projection_outcome()
                .map(|outcome| outcome.map_err(str::to_owned)),
            original,
            "the cached receipt retains the exact original projection result"
        );
        drop(events);
        Ok(())
    }
}

impl LoadoutReadObserver for Probe<'_> {
    fn with_record_paths(
        &self,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        self.capture.with_record_paths(record, tree, parse)
    }

    fn xml_path(
        &self,
        selection: &LoadoutSelection,
        site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        self.capture.xml_path(selection, site)
    }

    fn derived_path(
        &self,
        selection: &LoadoutSelection,
        inputs: &[&LoadoutPath],
        value: &str,
    ) -> ControlFlow<()> {
        self.capture.derived_path(selection, inputs, value)
    }

    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        {
            let mut events = self
                .events
                .lock()
                .expect("the source witness is not poisoned");
            match &event {
                LoadoutReadEvent::FileRead { observation, .. } => events.reads.push(Read {
                    source: observation.source().clone(),
                    kind: observation.kind(),
                    outcome: observation.read_outcome().map_err(Clone::clone),
                }),
                LoadoutReadEvent::FileProjection { observation, .. } => {
                    events.projections.push(Projection {
                        source: observation.source().clone(),
                        kind: observation.kind(),
                        outcome: observation
                            .projection_outcome()
                            .expect("an actual projection event carries its original result")
                            .map_err(str::to_owned),
                    });
                }
                LoadoutReadEvent::FileReused { observation, .. } => {
                    events
                        .reused
                        .push(LoadoutFileObservation::clone(observation));
                }
                _ => {}
            }
        }
        LoadoutReadObserver::observe(self.capture.as_ref(), event)
    }
}

impl CanonicalReadObserver for Probe<'_> {
    fn observe(
        &self,
        record: &Record<'_>,
        outcome: Result<&RecordTree<'_>, &DataForgeError>,
    ) -> ControlFlow<()> {
        CanonicalReadObserver::observe(self.capture.as_ref(), record, outcome)
    }
}

fn admission(source: &LoadoutFileSource, archive: &Archive, case: Case) -> TestResult {
    let selected = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == fixture::PATH);
    if matches!(case, Case::Missing) {
        assert!(
            selected.is_none(),
            "the configured target has no actual Archive Entry"
        );
        assert!(
            source.entry_origin().is_none(),
            "no invented admission for the missing file"
        );
        assert_eq!(
            source.identity().path,
            fixture::SELECTOR,
            "the genuine configured selector"
        );
        return Ok(());
    }
    let selected = selected.ok_or("actual original shared XML Entry")?;
    let original = archive
        .observed_entry(selected)
        .ok_or("actual original Entry binding")?;
    let admitted = source
        .entry_origin()
        .ok_or("the actual completed file admission")?;
    assert!(
        admitted.binding().is_same_entry(&original.binding()),
        "the observer's real source is the selected physical Archive Entry"
    );
    assert_eq!(
        source.identity().entry_index,
        Some(selected.index()),
        "actual Entry position"
    );
    assert_eq!(
        source.identity().path,
        selected.path(),
        "actual canonical Entry path"
    );
    Ok(())
}

fn original_outcomes(events: &Events, read: &Read, archive: &Archive, case: Case) -> TestResult {
    assert_eq!(
        read.kind,
        LoadoutDocumentKind::XmlLoadout,
        "actual intrinsic operation"
    );
    assert_eq!(
        read.outcome.is_ok(),
        case.read_ok(),
        "the actual original byte outcome"
    );
    if let Some(expected) = case.projected() {
        let projection = events
            .projections
            .first()
            .ok_or("actual intrinsic projection")?;
        admission(&projection.source, archive, case)?;
        assert_eq!(
            projection.kind, read.kind,
            "same original intrinsic document kind"
        );
        assert_eq!(
            projection.outcome.is_ok(),
            expected,
            "the original parser result"
        );
    }
    if matches!(case, Case::Missing) {
        assert_eq!(
            read.outcome,
            Err(LoadoutFileError::NotFound),
            "actual configured absence"
        );
    }
    Ok(())
}
