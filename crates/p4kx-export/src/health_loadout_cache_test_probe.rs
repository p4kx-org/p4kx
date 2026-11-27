// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_assembly::{LoadoutPathObserver, LoadoutReadEvent, LoadoutReadObserver};
use p4kx_extraction::{Record, RecordTree};

use crate::loadout::ledger::Capture;

pub(super) struct Probe<'input> {
    pub(super) capture: Arc<Capture<'input>>,
    reads: AtomicUsize,
    reused: AtomicUsize,
}

impl<'input> Probe<'input> {
    pub(super) const fn new(capture: Arc<Capture<'input>>) -> Self {
        Self {
            capture,
            reads: AtomicUsize::new(0),
            reused: AtomicUsize::new(0),
        }
    }

    pub(super) fn counts(&self, reads: usize, reused: usize) {
        assert_eq!(
            self.reads.load(Ordering::Relaxed),
            reads,
            "actual observed SDK FileRead callbacks"
        );
        assert_eq!(
            self.reused.load(Ordering::Relaxed),
            reused,
            "actual observed SDK FileReused callbacks"
        );
    }
}

impl LoadoutReadObserver for Probe<'_> {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        match &event {
            LoadoutReadEvent::FileRead { .. } => {
                self.reads.fetch_add(1, Ordering::Relaxed);
            }
            LoadoutReadEvent::FileReused { .. } => {
                self.reused.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        self.capture.observe(event)
    }

    fn with_record_paths(
        &self,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        self.capture.with_record_paths(record, tree, parse)
    }
}
