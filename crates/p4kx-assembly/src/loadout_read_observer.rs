// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_dataforge::{Record, RecordTree};

use super::LoadoutReadEvent;
use crate::{
    LoadoutClassSelection, LoadoutPath, LoadoutPathObserver, LoadoutRequest, LoadoutSelection,
    LoadoutXmlRecordSelector, VehicleImplementation, XmlLoadoutClassSite, XmlLoadoutPathSite,
};

/// Observes actual resolver source operations, including concurrent prefetch reads.
pub trait LoadoutReadObserver: Send + Sync {
    /// Returns `Break` to stop the observed resolution.
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()>;

    /// Enables original port-name selections in the resolver's read callbacks.
    fn observes_port_names(&self) -> bool {
        false
    }

    /// Enables exact DataForge selectors that supply an XML Loadout.
    fn observes_xml_record_selectors(&self) -> bool {
        false
    }

    /// Captures the original DataForge datum accepted by the XML Loadout path getter.
    fn xml_record_selector(&self, _site: &LoadoutXmlRecordSelector<'_, '_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    /// Enables exact XML evidence for effective vehicle visibility fields.
    fn observes_vehicle_visibility(&self) -> bool {
        false
    }

    /// Captures this Entity's actual vehicle caller, including uses of an existing cache entry.
    fn vehicle_used(
        &self,
        _record_index: u32,
        _implementation: &VehicleImplementation,
        _caller: &LoadoutRequest,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

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
