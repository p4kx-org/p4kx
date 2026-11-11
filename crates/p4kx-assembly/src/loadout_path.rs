// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{ControlFlow, Deref};
use std::sync::Arc;

use p4kx_dataforge::{RecordTree, StructValue, Value};

use crate::read;
use crate::resolver_observation::Observation;
use crate::{LoadoutClassName, LoadoutClassSelection, LoadoutError, LoadoutReferenceSelection};

/// A text operation supplying a file request or its selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LoadoutPathOperation {
    /// Copies the accepted source datum unchanged.
    Copy,
    /// Trims the accepted source datum.
    Trim,
    /// Confines the requested vehicle definition to its implementation directory.
    VehicleDefinition,
    /// Uses the definition's file stem in the implementation directory.
    VehicleDefinitionStem,
    /// Uses the Entity's class name in the implementation directory.
    VehicleClass,
    /// Selects an explicit patch by its original XML Modification name.
    VehicleModificationName,
    /// Resolves the selected Modification's explicit patch filename.
    VehiclePatch,
    /// Combines the definition's stem and selected Modification for a patch fallback.
    VehiclePatchStem,
    /// Uses the Entity's class name for a patch fallback.
    VehiclePatchClass,
}

/// An opaque handle minted by one actual request-path creator.
#[derive(Debug, Clone)]
pub struct LoadoutSelection {
    identity: Arc<()>,
    operation: LoadoutPathOperation,
}

impl LoadoutSelection {
    fn new(operation: LoadoutPathOperation) -> Self {
        Self {
            identity: Arc::new(()),
            operation,
        }
    }

    /// Returns the actual text operation of this selection.
    #[must_use]
    pub const fn operation(&self) -> LoadoutPathOperation {
        self.operation
    }
}

impl PartialEq for LoadoutSelection {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
}

impl Eq for LoadoutSelection {}

impl Hash for LoadoutSelection {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.identity).hash(state);
    }
}

/// A requested path paired with its actual source selection when observed.
#[derive(Debug, Clone)]
pub struct LoadoutPath {
    value: Arc<str>,
    selection: Option<LoadoutSelection>,
}

impl LoadoutPath {
    /// Creates a configured path without a source-authorship claim.
    #[must_use]
    pub fn unbound(value: impl Into<Arc<str>>) -> Self {
        Self {
            value: value.into(),
            selection: None,
        }
    }

    /// Borrows the exact path used by the reader.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Borrows the shared text without its source selection.
    #[must_use]
    pub const fn text(&self) -> &Arc<str> {
        &self.value
    }

    /// Borrows the original opaque selection of this whole request.
    #[must_use]
    pub const fn selection(&self) -> Option<&LoadoutSelection> {
        self.selection.as_ref()
    }

    pub(crate) fn xml(
        site: &crate::XmlLoadoutPathSite<'_>,
        observe: impl FnOnce(&LoadoutSelection) -> ControlFlow<()>,
    ) -> Result<Self, String> {
        let selection = LoadoutSelection::new(site.operation());
        if observe(&selection).is_break() {
            return Err(LoadoutError::ObservationStopped.to_string());
        }
        Ok(Self {
            value: Arc::from(site.value()),
            selection: Some(selection),
        })
    }

    pub(crate) fn derived(
        value: String,
        operation: LoadoutPathOperation,
        inputs: &[&Self],
        observation: Option<&Observation<'_>>,
    ) -> Result<Self, LoadoutError> {
        let Some(observation) = observation else {
            return Ok(Self::unbound(value));
        };
        let selection = LoadoutSelection::new(operation);
        observation.derived_path(&selection, inputs, &value)?;
        Ok(Self {
            value: Arc::from(value),
            selection: Some(selection),
        })
    }
}

impl PartialEq for LoadoutPath {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for LoadoutPath {}

impl Deref for LoadoutPath {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.value()
    }
}

impl fmt::Display for LoadoutPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.value())
    }
}

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

    /// Captures the actual Record name before its vehicle class fallback is copied.
    fn record_name(&self, selection: &LoadoutSelection, tree: &RecordTree<'_>) -> ControlFlow<()>;
}

pub(crate) struct PathRead<'scope> {
    observer: Option<&'scope dyn LoadoutPathObserver>,
    observation: Option<&'scope Observation<'scope>>,
    stopped: Cell<bool>,
}

impl<'scope> PathRead<'scope> {
    pub(crate) const fn new(
        observer: Option<&'scope dyn LoadoutPathObserver>,
        observation: Option<&'scope Observation<'scope>>,
    ) -> Self {
        Self {
            observer,
            observation,
            stopped: Cell::new(false),
        }
    }

    pub(crate) fn stopped(&self) -> bool {
        self.stopped.get() || self.observation.is_some_and(Observation::stopped)
    }

    fn stop(&self) {
        self.stopped.set(true);
        if let Some(observation) = self.observation {
            observation.stop();
        }
    }

    pub(crate) fn field(
        &self,
        parent: &StructValue<'_>,
        name: &str,
        operation: LoadoutPathOperation,
    ) -> LoadoutPath {
        let value = parent.field(name);
        let raw = value.map_or("", read::value_text);
        let text = match operation {
            LoadoutPathOperation::Trim => raw.trim(),
            _ => raw,
        };
        let selection = match (self.observer, value) {
            (Some(observer), Some(value)) if !raw.is_empty() && !self.stopped() => {
                let selection = LoadoutSelection::new(operation);
                if observer.record_path(&selection, parent, value).is_break() {
                    self.stop();
                }
                Some(selection)
            }
            _ => None,
        };
        LoadoutPath {
            value: Arc::from(text),
            selection,
        }
    }

    pub(crate) fn class_name(
        &self,
        parent: &StructValue<'_>,
        name: &str,
    ) -> Option<LoadoutClassName> {
        if self.stopped() {
            return None;
        }
        let value = parent.field(name)?;
        let text = read::value_text(value);
        if text.is_empty() {
            return None;
        }
        let selection = if let Some(observer) = self.observer {
            let selection = LoadoutClassSelection::new();
            if observer.record_class(&selection, parent, value).is_break() {
                self.stop();
                return None;
            }
            Some(selection)
        } else {
            None
        };
        Some(LoadoutClassName::captured(text, selection))
    }

    pub(crate) fn reference(
        &self,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> Option<LoadoutReferenceSelection> {
        let observer = self.observer?;
        if self.stopped() {
            return None;
        }
        let selection = LoadoutReferenceSelection::new();
        if observer
            .record_reference(&selection, parent, value)
            .is_break()
        {
            self.stop();
        }
        Some(selection)
    }

    pub(crate) fn record_name(&self, tree: &RecordTree<'_>) -> Option<LoadoutPath> {
        let observer = self.observer?;
        if self.stopped() {
            return None;
        }
        let selection = LoadoutSelection::new(LoadoutPathOperation::Copy);
        if observer.record_name(&selection, tree).is_break() {
            self.stop();
        }
        Some(LoadoutPath {
            value: Arc::from(tree.name()),
            selection: Some(selection),
        })
    }
}

#[cfg(test)]
#[path = "loadout_path_tests.rs"]
mod tests;
