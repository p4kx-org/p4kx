// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_assembly::{LoadoutFileObservation, LoadoutPath, LoadoutRequest, LoadoutSelection};

use crate::ExportAccountingError;
use crate::catalog_read::original::AuthoredEntryPath;

/// Retains recipes from original callbacks under their private operative handles.
#[derive(Default)]
pub(super) struct PathProofs {
    selections: HashMap<LoadoutSelection, AuthoredEntryPath>,
}

impl PathProofs {
    pub(super) fn insert(
        &mut self,
        selection: &LoadoutSelection,
        path: AuthoredEntryPath,
    ) -> Result<(), ExportAccountingError> {
        if self.selections.contains_key(selection) {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        self.selections.insert(selection.clone(), path);
        Ok(())
    }

    pub(super) fn get(
        &self,
        path: &LoadoutPath,
    ) -> Result<AuthoredEntryPath, ExportAccountingError> {
        self.lookup(path).cloned()
    }

    fn lookup(&self, path: &LoadoutPath) -> Result<&AuthoredEntryPath, ExportAccountingError> {
        let recipe = path
            .selection()
            .and_then(|selection| self.selections.get(selection))
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        if recipe.as_str() != path.value() {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        Ok(recipe)
    }

    pub(super) fn dependency<'request>(
        &self,
        parent: &LoadoutFileObservation,
        dependency: &'request LoadoutRequest,
    ) -> Result<(&'request LoadoutSelection, &AuthoredEntryPath), ExportAccountingError> {
        let LoadoutRequest::Xml { path } = dependency else {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        };
        let recipe = self.lookup(path)?;
        if !recipe.is_from_xml(parent) {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        let token = path
            .selection()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        Ok((token, recipe))
    }

    pub(super) fn request(
        &self,
        request: &LoadoutRequest,
    ) -> Result<Vec<AuthoredEntryPath>, ExportAccountingError> {
        request_paths(request)?.map(|path| self.get(path)).collect()
    }

    pub(super) fn validate_request(
        &self,
        request: &LoadoutRequest,
    ) -> Result<(), ExportAccountingError> {
        for path in request_paths(request)? {
            self.lookup(path)?;
        }
        Ok(())
    }
}

fn request_paths(
    request: &LoadoutRequest,
) -> Result<impl Iterator<Item = &LoadoutPath>, ExportAccountingError> {
    let paths = match request {
        LoadoutRequest::Xml { path } => [Some(path), None],
        LoadoutRequest::Vehicle {
            definition,
            modification,
            ..
        } => [
            (!definition.value().is_empty()).then_some(definition),
            (!modification.value().is_empty()).then_some(modification),
        ],
        _ => return Err(ExportAccountingError::ConflictingSourceDatum),
    };
    Ok(paths.into_iter().flatten())
}
