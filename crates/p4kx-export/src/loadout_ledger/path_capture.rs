// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_assembly::{
    LoadoutPath, LoadoutPathObserver, LoadoutPathOperation, LoadoutSelection, XmlLoadoutPathSite,
};
use p4kx_extraction::{Record, RecordTree};

use crate::catalog_read::original::{
    AuthoredEntryPath, EntryCapture, EntryPathCapture, LoadoutTargetCapture, OriginalFlowFields,
};
use crate::{CatalogError, ExportAccountingError};

use super::Capture;
use super::path_observer::RecordPaths;

impl Capture<'_> {
    pub(crate) fn port_name_proof(
        &self,
        name: &str,
        path: Option<&LoadoutPath>,
    ) -> Result<Option<AuthoredEntryPath>, CatalogError> {
        let Some(path) = path.filter(|path| path.selection().is_some()) else {
            return Ok(None);
        };
        if path.value() != name
            || path.selection().map(LoadoutSelection::operation)
                != Some(LoadoutPathOperation::PortName)
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let input = self
            .input
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let proof = self.lock()?.paths.get(path)?;
        proof.check_input(input)?;
        Ok(Some(proof))
    }

    pub(super) fn retain_path(
        &self,
        selection: &LoadoutSelection,
        path: AuthoredEntryPath,
    ) -> Result<(), CatalogError> {
        self.lock()?.paths.insert(selection, path)?;
        Ok(())
    }

    pub(super) fn trim_path(
        &self,
        path: AuthoredEntryPath,
    ) -> Result<AuthoredEntryPath, CatalogError> {
        let input = self
            .input
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        path.convert(&EntryCapture::for_input(input), |value| {
            Some(value.trim().to_owned())
        })?
        .ok_or_else(|| ExportAccountingError::ConflictingSourceDatum.into())
    }

    pub(super) fn capture_record_paths(
        &self,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        let Some(input) = self.input else {
            return parse(None);
        };
        if self.stopped() {
            return self.fail(CatalogError::Cancelled);
        }
        let result = (|| {
            let admission = self.admission(record)?;
            let scope = input.record_scope(record, tree)?;
            let paths = EntryPathCapture::new(input, &admission, &scope)?;
            let targets = LoadoutTargetCapture::from_paths(&paths);
            let observer = RecordPaths {
                capture: self,
                fields: OriginalFlowFields::read(&scope, self.cancel.as_ref())?,
                paths,
                targets,
            };
            Ok::<_, CatalogError>(parse(Some(&observer)))
        })();
        match result {
            Ok(flow) if !self.stopped() => flow,
            Ok(_) => ControlFlow::Break(()),
            Err(error) => self.fail(error),
        }
    }

    pub(super) fn capture_xml_path(
        &self,
        selection: &LoadoutSelection,
        site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        let Some(input) = self.input else {
            return ControlFlow::Continue(());
        };
        if self.stopped() {
            return self.fail(CatalogError::Cancelled);
        }
        let result = (|| {
            let observed = site.observation();
            if observed.read_outcome().is_err() || !self.lock()?.files.contains(observed) {
                return Err(ExportAccountingError::UnobservedFailure.into());
            }
            let capture = EntryCapture::for_input(input);
            let path = capture.loadout_xml(site)?;
            if !matches!(
                selection.operation(),
                LoadoutPathOperation::Copy
                    | LoadoutPathOperation::PortName
                    | LoadoutPathOperation::VehicleModificationName
            ) || selection.operation() != site.operation()
            {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            self.retain_path(selection, path)
        })();
        match result {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.fail(error),
        }
    }

    pub(super) fn capture_derived_path(
        &self,
        selection: &LoadoutSelection,
        inputs: &[&LoadoutPath],
        value: &str,
    ) -> ControlFlow<()> {
        let Some(input) = self.input else {
            return ControlFlow::Continue(());
        };
        if self.stopped() {
            return self.fail(CatalogError::Cancelled);
        }
        let result = (|| {
            match selection.operation() {
                LoadoutPathOperation::VehicleDefinition
                | LoadoutPathOperation::VehicleDefinitionStem
                | LoadoutPathOperation::VehicleClass
                | LoadoutPathOperation::VehiclePatch
                | LoadoutPathOperation::VehiclePatchStem
                | LoadoutPathOperation::VehiclePatchClass => {}
                _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
            }
            let paths = {
                let state = self.lock()?;
                inputs
                    .iter()
                    .map(|path| state.paths.get(path))
                    .collect::<Result<Vec<_>, _>>()?
            };
            let capture = EntryCapture::for_input(input);
            let path = if selection.operation() == LoadoutPathOperation::VehiclePatch {
                let [_, _, selected_name] = inputs else {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                };
                if selected_name.selection().map(LoadoutSelection::operation)
                    != Some(LoadoutPathOperation::VehicleModificationName)
                {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
                let [file, request, name] = paths.as_slice() else {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                };
                AuthoredEntryPath::derived(&capture, &[file], value)?
                    .selected_by(request)?
                    .selected_by(name)?
            } else {
                let references: Vec<_> = paths.iter().collect();
                AuthoredEntryPath::derived(&capture, &references, value)?
            };
            self.retain_path(selection, path)
        })();
        match result {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.fail(error),
        }
    }
}
