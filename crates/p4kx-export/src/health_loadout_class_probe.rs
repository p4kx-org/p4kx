// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use p4kx_assembly::{
    LoadoutClassSelection, LoadoutPath, LoadoutPathObserver, LoadoutReadEvent, LoadoutReadObserver,
    LoadoutSelection, XmlLoadoutClassSite, XmlLoadoutPathSite,
};
use p4kx_extraction::{Record, RecordTree};

use crate::Kind;
use crate::build::original_input::OriginalCatalogInput;
use crate::export_ledger::ExportLedger;
use crate::loadout::ledger::Capture;

pub(super) struct Probe<'input> {
    pub(super) capture: Arc<Capture<'input>>,
    pub(super) selections: Mutex<Vec<LoadoutClassSelection>>,
    reads: AtomicUsize,
    reused: AtomicUsize,
    edges: AtomicUsize,
}

impl<'input> Probe<'input> {
    pub(super) fn new(input: &'input OriginalCatalogInput<'input, 'input>) -> Self {
        Self {
            capture: Arc::new(Capture::for_input(
                input,
                &ExportLedger::default(),
                None,
                Kind::Loadouts,
            )),
            selections: Mutex::new(Vec::new()),
            reads: AtomicUsize::new(0),
            reused: AtomicUsize::new(0),
            edges: AtomicUsize::new(0),
        }
    }
}

impl Probe<'_> {
    pub(super) fn file_counts(&self, reads: usize, reused: usize, edges: usize) {
        assert_eq!(
            self.reads.load(Ordering::Relaxed),
            reads,
            "actual SDK original file reads"
        );
        assert_eq!(
            self.reused.load(Ordering::Relaxed),
            reused,
            "actual SDK root cache reuses"
        );
        assert_eq!(
            self.edges.load(Ordering::Relaxed),
            edges,
            "actual SDK direct Include edges"
        );
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

    fn xml_class(
        &self,
        selection: &LoadoutClassSelection,
        site: &XmlLoadoutClassSite<'_>,
    ) -> ControlFlow<()> {
        self.capture.xml_class(selection, site)
    }

    fn class_record(
        &self,
        selection: &LoadoutClassSelection,
        record: &Record<'_>,
    ) -> ControlFlow<()> {
        let outcome = self.capture.class_record(selection, record);
        if outcome.is_continue() {
            assert_eq!(
                record.index(),
                0,
                "the actual SDK callback supplies the original first definition"
            );
            self.selections
                .lock()
                .expect("class source log")
                .push(selection.clone());
        }
        outcome
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
        match &event {
            LoadoutReadEvent::FileRead { .. } => {
                self.reads.fetch_add(1, Ordering::Relaxed);
            }
            LoadoutReadEvent::FileReused { .. } => {
                self.reused.fetch_add(1, Ordering::Relaxed);
            }
            LoadoutReadEvent::FileDependency { .. } => {
                self.edges.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        self.capture.observe(event)
    }
}
