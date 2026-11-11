// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Entry};

use crate::directory_lookup::DirectoryLookup;
use crate::source_selection::EntrySelection;
use crate::{MediaError, MediaWriter};

/// Work performed at the common bounded Archive traversal seam.
pub(crate) trait TraversalVisit {
    /// Whether physical inputs belong to the exported source tally.
    fn counts_sources(&self) -> bool;

    fn directory(
        &mut self,
        archive: &Archive,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError>;

    fn entry(
        &mut self,
        entry: &Entry,
        selection: EntrySelection,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError>;

    fn directory_complete(
        &mut self,
        _archive: &Archive,
        _lookup: &DirectoryLookup<'_>,
        _writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        Ok(())
    }

    fn excluded_container(&mut self);
}
