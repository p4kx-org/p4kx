// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_assembly::{
    LoadoutClassSelection, LoadoutPathObserver, LoadoutPathOperation, LoadoutReferenceSelection,
    LoadoutSelection,
};
use p4kx_extraction::{RecordTree, StructValue, Value};

use crate::catalog_read::original::{
    EntryPathCapture, LoadoutClassNameReceipt, LoadoutTargetCapture, OriginalFlowFields,
    OriginalValueFact,
};
use crate::{CatalogError, ExportAccountingError};

use super::Capture;

pub(super) struct RecordPaths<'capture, 'input, 'scope, 'read, 'source> {
    pub(super) capture: &'capture Capture<'input>,
    pub(super) fields: OriginalFlowFields<'scope, 'read, 'source>,
    pub(super) paths: EntryPathCapture<'scope, 'read, 'source>,
    pub(super) targets: LoadoutTargetCapture<'scope, 'read, 'source>,
}

impl<'scope, 'read, 'source> RecordPaths<'_, '_, 'scope, 'read, 'source> {
    fn fact(
        &self,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> Result<&OriginalValueFact<'scope, 'read, 'source>, CatalogError> {
        let fact = self.fields.fact(value)?;
        if !std::ptr::addr_eq(self.fields.guard_parent(fact)?, parent) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(fact)
    }

    fn reference(
        &self,
        selection: &LoadoutReferenceSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> Result<(), CatalogError> {
        let fact = self.fact(parent, value)?;
        let ancestors = self.fields.ancestors(fact)?;
        let target = self
            .targets
            .value_fact(self.fields.guard_parent(fact)?, fact, &ancestors)?;
        self.capture.retain_reference(selection, target)
    }

    fn class(
        &self,
        selection: &LoadoutClassSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> Result<(), CatalogError> {
        let fact = self.fact(parent, value)?;
        let ancestors = self.fields.ancestors(fact)?;
        let name = LoadoutClassNameReceipt::record(
            &self.paths,
            self.fields.guard_parent(fact)?,
            fact,
            &ancestors,
        )?;
        self.capture.retain_class(selection, name)
    }

    fn path(
        &self,
        selection: &LoadoutSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> Result<(), CatalogError> {
        let fact = self.fact(parent, value)?;
        let original_parent = self.fields.guard_parent(fact)?;
        let ancestors = self.fields.ancestors(fact)?;
        let path = self.paths.value_fact(original_parent, fact, &ancestors)?;
        let path = match selection.operation() {
            LoadoutPathOperation::Copy => path,
            LoadoutPathOperation::Trim => self.capture.trim_path(path)?,
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        self.capture.retain_path(selection, path)
    }
}

impl LoadoutPathObserver for RecordPaths<'_, '_, '_, '_, '_> {
    fn record_reference(
        &self,
        selection: &LoadoutReferenceSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> ControlFlow<()> {
        if self.capture.stopped() {
            return self.capture.fail(CatalogError::Cancelled);
        }
        match self.reference(selection, parent, value) {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.capture.fail(error),
        }
    }

    fn record_class(
        &self,
        selection: &LoadoutClassSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> ControlFlow<()> {
        if self.capture.stopped() {
            return self.capture.fail(CatalogError::Cancelled);
        }
        match self.class(selection, parent, value) {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.capture.fail(error),
        }
    }

    fn record_path(
        &self,
        selection: &LoadoutSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> ControlFlow<()> {
        if self.capture.stopped() {
            return self.capture.fail(CatalogError::Cancelled);
        }
        match self.path(selection, parent, value) {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.capture.fail(error),
        }
    }

    fn record_name(&self, selection: &LoadoutSelection, tree: &RecordTree<'_>) -> ControlFlow<()> {
        if self.capture.stopped() {
            return self.capture.fail(CatalogError::Cancelled);
        }
        let result = self
            .paths
            .record_name(tree)
            .and_then(|path| self.capture.retain_path(selection, path));
        match result {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.capture.fail(error),
        }
    }
}
