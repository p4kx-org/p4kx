// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_assembly::{
    LoadoutFileObservation, LoadoutPath, LoadoutPathObserver, LoadoutReadEvent,
    LoadoutReadObserver, LoadoutSelection, XmlLoadoutPathSite,
};
use p4kx_extraction::{Record, RecordTree};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry};

use crate::loadout::ledger::Capture;

pub(super) const VEHICLE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Context.xml";
pub(super) const PATCH: &str =
    "Scripts/Entities/Vehicles/Implementations/Xml/Modifications/context.xml";
const PATCH_ELEMENTS: usize = 1 << 12;

pub(super) fn database() -> Vec<u8> {
    p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(71, "ContextVehicle", "synthetic/context.xml")
                .vehicle(VEHICLE, "Budgeted")
                .record(),
        )
        .build()
}

pub(super) fn entries() -> Vec<TestEntry> {
    let base = br#"<Vehicle><Modifications><Modification name="Budgeted" patchFile="Modifications/context.xml"/></Modifications></Vehicle>"#;
    let patch = format!("<Patch>{}</Patch>", "<x/>".repeat(PATCH_ELEMENTS));
    vec![
        TestEntry::stored(format!("Data/{VEHICLE}"), base.as_slice()),
        TestEntry::stored(format!("Data/{PATCH}"), patch.into_bytes()),
    ]
}

#[derive(Default)]
pub(super) struct Seen {
    pub(super) reads: usize,
    pub(super) projections: usize,
    pub(super) incomplete: usize,
    pub(super) reused: Vec<LoadoutFileObservation>,
}

pub(super) struct Probe<'input> {
    pub(super) capture: Arc<Capture<'input>>,
    pub(super) seen: Mutex<Seen>,
}

impl<'input> Probe<'input> {
    pub(super) fn new(capture: Arc<Capture<'input>>) -> Self {
        Self {
            capture,
            seen: Mutex::new(Seen::default()),
        }
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
            let mut seen = self.seen.lock().expect("actual file observation mutex");
            match &event {
                LoadoutReadEvent::FileRead { .. } => seen.reads += 1,
                LoadoutReadEvent::FileProjection { .. } => seen.projections += 1,
                LoadoutReadEvent::InventoryIncomplete => seen.incomplete += 1,
                LoadoutReadEvent::FileReused { observation, .. } => {
                    seen.reused.push(LoadoutFileObservation::clone(observation));
                }
                _ => {}
            }
            drop(seen);
        }
        LoadoutReadObserver::observe(self.capture.as_ref(), event)
    }
}
