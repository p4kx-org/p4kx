// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, Entry};

use crate::{
    FieldOrigin, InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

pub(crate) struct EntryDirectorySource {
    pub(crate) path: FieldOrigin,
    pub(crate) path_uses: Vec<SourceUse>,
    directory: SourceScope,
    uses: SourceUses,
    complete: bool,
}

impl EntryDirectorySource {
    pub(crate) fn observe(
        archive: &Archive,
        entry: &Entry,
        uses: SourceUses,
    ) -> Result<Self, SourceEvidenceError> {
        let observed = archive.observed_entry(entry);
        let input = observed.map_or(SourceInput::Unknown, SourceInput::ArchiveView);
        let ancestors = observed.map_or(0, |entry| entry.ancestors().len());
        let complete = observed.is_some_and(|entry| entry.ancestors().all(|step| step.is_some()));
        let directory = uses.observe(input, InputScope::ArchiveDirectoryMetadata)?;
        let (path_uses, complete_path) = directory_path_uses(&directory, ancestors, complete)?;
        let path = uses.origin(&path_uses, complete_path);
        Ok(Self {
            path,
            path_uses,
            directory,
            uses,
            complete,
        })
    }

    pub(crate) fn directory_subresource(
        &self,
        location: SourceLocation,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.directory.at(location, role)
    }

    pub(crate) fn directory_fact(
        &self,
        field: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.directory.at(metadata(field), role)
    }

    pub(crate) fn origin(&self, actual: &[SourceUse]) -> FieldOrigin {
        self.origin_when(actual, true)
    }

    pub(crate) fn origin_when(&self, actual: &[SourceUse], complete: bool) -> FieldOrigin {
        self.uses.origin(actual, self.complete && complete)
    }

    pub(crate) const fn complete(&self) -> bool {
        self.complete
    }
}

/// Captures the original outer-to-inner directory paths leading to an Entry.
///
/// # Errors
/// Returns an error when an original path use cannot be registered.
pub fn directory_path_uses(
    directory: &SourceScope,
    ancestors: usize,
    mut complete: bool,
) -> Result<(Vec<SourceUse>, bool), SourceEvidenceError> {
    let mut path = Vec::new();
    for index in 0..ancestors {
        if let Some(ancestor) = directory.ancestor(index)? {
            path.push(ancestor.at(metadata("path"), SourceUseRole::Payload)?);
        } else {
            complete = false;
        }
    }
    path.push(directory.at(metadata("path"), SourceUseRole::Payload)?);
    Ok((path, complete))
}

fn metadata(field: &str) -> SourceLocation {
    SourceLocation::Metadata {
        field: field.into(),
    }
}
