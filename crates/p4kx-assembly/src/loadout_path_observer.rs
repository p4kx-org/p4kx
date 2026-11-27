// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_dataforge::{RecordTree, StructValue, Value};

use super::LoadoutSelection;
use crate::{ItemClassification, LoadoutClassSelection, LoadoutReferenceSelection, PortDefinition};

/// Receives actual Record path getters while their original tree remains borrowed.
pub trait LoadoutPathObserver {
    /// Captures the actual parent and accepted datum before its path text is copied.
    fn record_path(
        &self,
        selection: &LoadoutSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> ControlFlow<()>;

    /// Captures an accepted original Reference before its GUID is copied.
    fn record_reference(
        &self,
        _selection: &LoadoutReferenceSelection,
        _parent: &StructValue<'_>,
        _value: &Value<'_>,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    /// Captures an accepted original class name before its text is copied.
    fn record_class(
        &self,
        _selection: &LoadoutClassSelection,
        _parent: &StructValue<'_>,
        _value: &Value<'_>,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    /// Captures the resolved definition and its original struct during the same Record read.
    fn record_port_definition(
        &self,
        _definition: &PortDefinition,
        _original: &StructValue<'_>,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    /// Captures the selected item type while its original attachable component is borrowed.
    fn record_item_type(
        &self,
        _component: Option<(usize, &StructValue<'_>)>,
        _classification: &ItemClassification,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    /// Captures the chosen inventory declaration before its reference is copied.
    fn record_inventory_container(
        &self,
        _component: Option<&StructValue<'_>>,
        _value: Option<&Value<'_>>,
        _guid: Option<p4kx_dataforge::Guid>,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    /// Binds a chosen empty manual entry to its actual original Record struct.
    fn record_empty_loadout_entry(
        &self,
        _selection: &LoadoutSelection,
        _entry: &StructValue<'_>,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    /// Captures the actual Record name before its vehicle class fallback is copied.
    fn record_name(&self, selection: &LoadoutSelection, tree: &RecordTree<'_>) -> ControlFlow<()>;
}
