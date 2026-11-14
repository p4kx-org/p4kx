// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use p4kx_assembly::{
    LoadoutFileObservation, LoadoutFileOperationKey, LoadoutSelection, LoadoutXmlClassUse,
};

use super::AuthoredEntryPath;
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError};

#[path = "health_loadout_graph_state.rs"]
mod state;

struct Operation {
    original: LoadoutFileObservation,
    candidate: AuthoredEntryPath,
    first_caller: Vec<AuthoredEntryPath>,
    first_parent: Option<usize>,
    first_xml_caller: Option<LoadoutSelection>,
    reused_xml_callers: HashMap<LoadoutSelection, AuthoredEntryPath>,
}

struct IncludeEdge {
    parent: usize,
    child: usize,
    selection: AuthoredEntryPath,
}

#[derive(Default)]
struct State {
    operations: Vec<Operation>,
    positions: HashMap<LoadoutFileOperationKey, usize>,
    edges: HashMap<LoadoutSelection, IncludeEdge>,
    sealed: bool,
}

/// One resolver's actual original operations and direct selection edges.
#[derive(Clone, Default)]
pub(crate) struct FileGraph {
    state: Arc<Mutex<State>>,
}

/// A private routing handle whose operation still owns the genuine immutable read proof.
#[derive(Clone)]
pub(crate) struct FileSelection {
    graph: FileGraph,
    operation: usize,
}

/// One observed route to the actual operation that supplied a class getter.
#[derive(Debug, Clone)]
pub(crate) struct ClassXmlSelection {
    original: LoadoutFileObservation,
    paths: Arc<[AuthoredEntryPath]>,
}

impl ClassXmlSelection {
    pub(super) fn belongs_to(&self, original: &LoadoutFileObservation) -> bool {
        self.original.is_same_operation(original)
    }

    pub(super) fn paths(&self) -> &[AuthoredEntryPath] {
        &self.paths
    }
}

impl FileGraph {
    pub(crate) fn selection(
        &self,
        original: &LoadoutFileObservation,
    ) -> Result<FileSelection, CatalogError> {
        let state = crate::export_evidence_lock::lock(&self.state)?;
        let operation = state
            .position(original)
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        drop(state);
        Ok(FileSelection {
            graph: self.clone(),
            operation,
        })
    }

    pub(crate) fn read(
        &self,
        original: &LoadoutFileObservation,
        candidate: AuthoredEntryPath,
        caller: Vec<AuthoredEntryPath>,
        xml_caller: Option<&LoadoutSelection>,
    ) -> Result<FileSelection, CatalogError> {
        let mut state = crate::export_evidence_lock::lock(&self.state)?;
        state.open()?;
        if state.position(original).is_some() {
            return Err(ExportAccountingError::UnobservedFailure.into());
        }
        let operation = state.operations.len();
        if xml_caller.is_some() && caller.len() != 1 {
            return Err(ExportAccountingError::UnobservedFailure.into());
        }
        state.positions.insert(original.operation_key(), operation);
        state.operations.push(Operation {
            original: original.clone(),
            candidate,
            first_caller: caller,
            first_parent: None,
            first_xml_caller: xml_caller.cloned(),
            reused_xml_callers: HashMap::new(),
        });
        drop(state);
        Ok(FileSelection {
            graph: self.clone(),
            operation,
        })
    }

    pub(crate) fn selected(
        &self,
        original: &LoadoutFileObservation,
        caller: &LoadoutSelection,
        recipe: AuthoredEntryPath,
    ) -> Result<(), CatalogError> {
        let mut state = crate::export_evidence_lock::lock(&self.state)?;
        state.open()?;
        let operation = state
            .position(original)
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        let operation = state.operation_mut(operation)?;
        if let Some(original) = operation.xml_caller(caller) {
            if !original.is_same_recipe(&recipe) {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
        } else {
            operation.reused_xml_callers.insert(caller.clone(), recipe);
        }
        drop(state);
        Ok(())
    }

    pub(crate) fn check_candidate(
        &self,
        original: &LoadoutFileObservation,
        candidate: &AuthoredEntryPath,
    ) -> Result<(), CatalogError> {
        let state = crate::export_evidence_lock::lock(&self.state)?;
        let operation = state
            .position(original)
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        let matches = state
            .operation(operation)?
            .candidate
            .is_same_recipe(candidate);
        drop(state);
        if !matches {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(())
    }

    pub(crate) fn check_read_context(
        &self,
        original: &LoadoutFileObservation,
        candidate: &AuthoredEntryPath,
        caller: &[AuthoredEntryPath],
    ) -> Result<(), CatalogError> {
        let state = crate::export_evidence_lock::lock(&self.state)?;
        let position = state
            .position(original)
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        let operation = state.operation(position)?;
        let matches = operation.candidate.is_same_recipe(candidate)
            && same_request(&operation.first_caller, caller);
        drop(state);
        if !matches {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(())
    }

    pub(crate) fn dependency(
        &self,
        parent: &LoadoutFileObservation,
        child: &LoadoutFileObservation,
        token: &LoadoutSelection,
        selection: &AuthoredEntryPath,
    ) -> Result<(), CatalogError> {
        let mut state = crate::export_evidence_lock::lock(&self.state)?;
        state.open()?;
        let parent = state
            .position(parent)
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        let child = state
            .position(child)
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        if state.edges.get(token).is_some_and(|observed| {
            observed.parent != parent
                || observed.child != child
                || !observed.selection.is_same_recipe(selection)
        }) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let operation = state.operation_mut(child)?;
        if same_request(&operation.first_caller, std::slice::from_ref(selection)) {
            if operation
                .first_parent
                .is_some_and(|original| original != parent)
            {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            operation.first_parent = Some(parent);
        }
        state
            .edges
            .entry(token.clone())
            .or_insert_with(|| IncludeEdge {
                parent,
                child,
                selection: selection.clone(),
            });
        drop(state);
        Ok(())
    }

    pub(crate) fn selected_xml_use(
        &self,
        usage: &LoadoutXmlClassUse,
        getter: &LoadoutFileObservation,
    ) -> Result<ClassXmlSelection, ExportAccountingError> {
        let state = crate::export_evidence_lock::lock(&self.state)?;
        let index = |original| {
            state
                .position(original)
                .ok_or(ExportAccountingError::UnobservedFailure)
        };
        let mut current = index(usage.original_root())?;
        let caller = usage
            .caller()
            .selection()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let caller = state
            .operation(current)?
            .xml_caller(caller)
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        let mut paths = vec![caller.clone()];
        for (parent, child, path) in usage.includes() {
            let parent = index(parent)?;
            let child = index(child)?;
            let token = path
                .selection()
                .ok_or(ExportAccountingError::UnboundHealthSubject)?;
            let edge = state
                .edges
                .get(token)
                .ok_or(ExportAccountingError::UnobservedFailure)?;
            if parent != current || edge.parent != parent || edge.child != child {
                return Err(ExportAccountingError::ConflictingSourceDatum);
            }
            paths.push(edge.selection.clone());
            current = child;
        }
        if current != index(getter)? {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        drop(state);
        Ok(ClassXmlSelection {
            original: getter.clone(),
            paths: Arc::from(paths),
        })
    }

    pub(crate) fn refused(
        &self,
        parent: &LoadoutFileObservation,
        token: &LoadoutSelection,
    ) -> Result<(), CatalogError> {
        let state = crate::export_evidence_lock::lock(&self.state)?;
        state.open()?;
        state
            .position(parent)
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        if state.edges.contains_key(token) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        drop(state);
        Ok(())
    }
}

impl Operation {
    fn xml_caller(&self, token: &LoadoutSelection) -> Option<&AuthoredEntryPath> {
        if self.first_xml_caller.as_ref() == Some(token) {
            self.first_caller.first()
        } else {
            self.reused_xml_callers.get(token)
        }
    }
}

impl FileSelection {
    pub(crate) fn original(&self) -> Result<LoadoutFileObservation, CatalogError> {
        let state = crate::export_evidence_lock::lock(&self.graph.state)?;
        let original = state.operation(self.operation)?.original.clone();
        drop(state);
        Ok(original)
    }

    pub(crate) fn candidate(&self) -> Result<AuthoredEntryPath, CatalogError> {
        let state = crate::export_evidence_lock::lock(&self.graph.state)?;
        let candidate = state.operation(self.operation)?.candidate.clone();
        drop(state);
        Ok(candidate)
    }

    pub(super) fn check_initial_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        let state = crate::export_evidence_lock::lock(&self.graph.state)?;
        state
            .operation(self.operation)?
            .candidate
            .check_input(input)?;
        for selector in state.initial_selectors(self.operation)? {
            selector.check_input(input)?;
        }
        drop(state);
        Ok(())
    }

    pub(super) fn initial_selectors(&self) -> Result<Vec<AuthoredEntryPath>, CatalogError> {
        let mut state = crate::export_evidence_lock::lock(&self.graph.state)?;
        state.sealed = true;
        let selected = state
            .initial_selectors(self.operation)?
            .into_iter()
            .cloned()
            .collect();
        drop(state);
        Ok(selected)
    }
}

impl fmt::Debug for FileSelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileSelection")
            .finish_non_exhaustive()
    }
}

fn same_request(left: &[AuthoredEntryPath], right: &[AuthoredEntryPath]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.is_same_recipe(right))
}
