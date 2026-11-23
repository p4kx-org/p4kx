// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, hash_map};

use p4kx_extraction::{Archive, FieldOrigin, SourceValue, SourcedValue};

use crate::catalog_read::original::{EntryCapture, EntryReceipt};
use crate::export_archive_health;
use crate::starmap_index::{self, Index};
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
        index: &Index<'_>,
    ) -> Result<Self, CatalogError> {
        capture.check_archive(archive.input_origin())?;
        if !std::ptr::eq(index.entries, archive.entries()) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
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
        let group = self.at(opened.system_entry)?;
        if group.value().map(String::as_str) != Some(opened.system.as_ref()) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(group.clone())
    }

    /// Resolves an exact physical system anchor in this Archive directory.
    pub(super) fn at(&mut self, slot: usize) -> Result<&SourcedValue<String>, CatalogError> {
        match self.by_entry.entry(slot) {
            hash_map::Entry::Occupied(found) => Ok(found.into_mut()),
            hash_map::Entry::Vacant(vacant) => Ok(vacant.insert(system_folder(
                self.archive,
                self.capture,
                slot,
                &self.directory,
            )?)),
        }
    }
}

fn system_folder(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    slot: usize,
    directory: &FieldOrigin,
) -> Result<SourcedValue<String>, CatalogError> {
    let sources = capture.source_uses();
    let Some(entry) = archive.entries().get(slot) else {
        let count = sources.observe_root_directory_entry_count(archive)?;
        let complete = directory.source_set_complete() && count.is_some();
        let range = sources.origin(count.as_slice(), complete);
        let outcome = if complete {
            SourceValue::Absent
        } else {
            SourceValue::Unreadable
        };
        return Ok(sources.value_with_origins(outcome, &[directory, &range])?);
    };
    if entry.index() != slot {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let path = EntryReceipt::outer(capture, archive, entry)?.physical_file(starmap_index::key)?;
    let normalized = path
        .normalized_path()
        .value()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let outcome = starmap_index::system_container_folder(normalized)
        .map_or(SourceValue::Invalid, |folder| {
            SourceValue::Present(folder.to_owned())
        });
    Ok(sources.value_with_origins(
        outcome,
        &[
            path.source_path().origin(),
            path.normalized_path().origin(),
            directory,
        ],
    )?)
}
