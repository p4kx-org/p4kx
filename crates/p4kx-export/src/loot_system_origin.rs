// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, hash_map};

use p4kx_extraction::{Archive, FieldOrigin, SourceValue, SourcedValue};

use crate::catalog_read::original::{EntryCapture, EntryReceipt};
use crate::export_archive_health;
use crate::starmap_index;
use crate::starmap_walk::Opened;
use crate::{CatalogError, ExportAccountingError};

/// Projects folder values from the original directory slots of winning occurrences.
pub(super) struct SystemOrigins<'read, 'input> {
    archive: &'read Archive,
    capture: &'read EntryCapture<'input>,
    directory: FieldOrigin,
    by_entry: HashMap<usize, SourcedValue<String>>,
}

impl<'read, 'input> SystemOrigins<'read, 'input> {
    pub(super) fn try_new(
        archive: &'read Archive,
        capture: &'read EntryCapture<'input>,
    ) -> Result<Self, CatalogError> {
        let sources = capture.source_uses();
        let survey = sources.observe_root_directory_paths(archive)?;
        let complete = survey.is_some() && export_archive_health::inventory(archive).is_none();
        Ok(Self {
            archive,
            capture,
            directory: sources.origin(survey.as_slice(), complete),
            by_entry: HashMap::new(),
        })
    }

    pub(super) fn project(
        &mut self,
        opened: &Opened,
    ) -> Result<SourcedValue<String>, CatalogError> {
        let group = match self.by_entry.entry(opened.system_entry) {
            hash_map::Entry::Occupied(found) => found.into_mut(),
            hash_map::Entry::Vacant(vacant) => vacant.insert(system_folder(
                self.archive,
                self.capture,
                opened.system_entry,
                &self.directory,
            )?),
        };
        if group.value().map(String::as_str) != Some(opened.system.as_ref()) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(group.clone())
    }
}

fn system_folder(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    slot: usize,
    directory: &FieldOrigin,
) -> Result<SourcedValue<String>, CatalogError> {
    let entry = archive
        .entries()
        .get(slot)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let path = EntryReceipt::outer(capture, archive, entry)?.physical_file(starmap_index::key)?;
    let normalized = path
        .normalized_path()
        .value()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let (system, _) = starmap_index::system_path_parts(normalized)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    Ok(capture.source_uses().value_with_origins(
        SourceValue::Present(system.to_owned()),
        &[
            path.source_path().origin(),
            path.normalized_path().origin(),
            directory,
        ],
    )?)
}
