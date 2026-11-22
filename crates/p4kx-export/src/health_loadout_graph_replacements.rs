// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_assembly::{
    LoadoutFileObservation, LoadoutSelection, XmlPortReplacement, XmlPortSelection,
};

use super::{AuthoredEntryPath, FileGraph, State};
use crate::ExportAccountingError;

impl FileGraph {
    pub(crate) fn xml_replacement_selectors(
        &self,
        root: &LoadoutFileObservation,
        request: &LoadoutSelection,
        replacement: &XmlPortReplacement,
        operands: [&AuthoredEntryPath; 2],
    ) -> Result<Vec<AuthoredEntryPath>, ExportAccountingError> {
        let state = crate::export_evidence_lock::lock(&self.state)?;
        let root = xml_position(&state, root)?;
        let merge = xml_position(&state, replacement.merge_document())?;
        let caller = state
            .operation(root)?
            .xml_caller(request)
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        let mut selected = vec![caller.clone()];
        selected.extend(state.merge_ancestry(root, merge)?.into_iter().cloned());
        for (operand, getter) in [replacement.displaced(), replacement.winner()]
            .into_iter()
            .zip(operands)
        {
            state.xml_port_selectors(merge, operand, getter, &mut selected)?;
        }
        drop(state);
        Ok(selected)
    }
}

impl State {
    fn merge_ancestry(
        &self,
        root: usize,
        merge: usize,
    ) -> Result<Vec<&AuthoredEntryPath>, ExportAccountingError> {
        let mut seen = HashSet::new();
        let mut current = merge;
        let mut selected = Vec::new();
        while current != root {
            if !seen.insert(current) {
                return Err(ExportAccountingError::ConflictingSourceDatum);
            }
            let operation = self.operation(current)?;
            let parent = operation
                .first_parent
                .ok_or(ExportAccountingError::UnobservedFailure)?;
            let caller = operation
                .first_xml_caller
                .as_ref()
                .ok_or(ExportAccountingError::UnboundHealthSubject)?;
            selected.push(self.xml_edge(parent, current, caller)?);
            current = parent;
        }
        selected.reverse();
        Ok(selected)
    }

    fn xml_port_selectors(
        &self,
        merge: usize,
        operand: &XmlPortSelection,
        getter: &AuthoredEntryPath,
        selected: &mut Vec<AuthoredEntryPath>,
    ) -> Result<(), ExportAccountingError> {
        let mut current = merge;
        for (parent, child, path) in operand.includes() {
            let parent = xml_position(self, parent)?;
            let child = xml_position(self, child)?;
            if parent != current {
                return Err(ExportAccountingError::ConflictingSourceDatum);
            }
            let token = path
                .selection()
                .ok_or(ExportAccountingError::UnboundHealthSubject)?;
            selected.push(self.xml_edge(parent, child, token)?.clone());
            current = child;
        }
        if !getter.is_from_xml(&self.operation(current)?.original) {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        selected.push(getter.clone());
        Ok(())
    }

    pub(super) fn xml_edge(
        &self,
        parent: usize,
        child: usize,
        caller: &LoadoutSelection,
    ) -> Result<&AuthoredEntryPath, ExportAccountingError> {
        let edge = self
            .edges
            .get(caller)
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        if edge.parent != parent || edge.child != child {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        Ok(&edge.selection)
    }
}

fn xml_position(
    state: &State,
    original: &LoadoutFileObservation,
) -> Result<usize, ExportAccountingError> {
    state
        .position(original)
        .ok_or(ExportAccountingError::UnobservedFailure)
}
