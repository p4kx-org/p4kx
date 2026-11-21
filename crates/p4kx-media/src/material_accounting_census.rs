// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Entry};

use crate::source_selection::EntrySelection;
use crate::{ContainerIdentity, Identity, MediaError};

#[derive(Default)]
pub(super) struct Census {
    directories: Vec<Directory>,
    chain: Vec<ContainerIdentity>,
    root_complete: bool,
    counts: CensusCounts,
}

struct Directory {
    entries: usize,
    next: usize,
    pending_child: Option<ContainerIdentity>,
}

#[derive(Default)]
pub(super) struct CensusCounts {
    pub(super) enumerated: u64,
    pub(super) selected: u64,
}

impl Census {
    pub(super) fn open(
        &mut self,
        archive: &Archive,
        chain: &[ContainerIdentity],
    ) -> Result<(), MediaError> {
        if self.root_complete
            || archive
                .issues()
                .iter()
                .any(|(_, issue)| issue.is_archive_wide())
        {
            return Err(invalid(
                "material enumeration crossed an unexpected or incomplete archive",
            ));
        }
        if let Some(parent) = self.directories.last_mut() {
            let child = parent
                .pending_child
                .take()
                .ok_or_else(|| invalid("material enumeration entered an unexpected container"))?;
            self.chain.push(child);
        }
        if self.chain != chain {
            return Err(invalid(
                "material container ancestry differs from the physical directory",
            ));
        }
        self.directories.push(Directory {
            entries: archive.entries().len(),
            next: 0,
            pending_child: None,
        });
        Ok(())
    }

    pub(super) fn entry(
        &mut self,
        archive: &Archive,
        entry: &Entry,
        selection: EntrySelection,
    ) -> Result<Option<Identity>, MediaError> {
        let directory = self
            .directories
            .last_mut()
            .ok_or_else(|| invalid("material entry appeared outside a directory"))?;
        if directory.pending_child.is_some() {
            return Err(invalid("material traversal skipped a physical container"));
        }
        if !archive
            .entries()
            .get(directory.next)
            .is_some_and(|expected| std::ptr::eq(expected, entry))
        {
            return Err(invalid(
                "material traversal skipped or reordered a physical entry",
            ));
        }
        directory.next += 1;
        self.counts.enumerated += 1;

        let path = entry.path();
        let extension = path.rsplit_once('.').map(|(_, extension)| extension);
        let selected = extension.is_some_and(|extension| extension.eq_ignore_ascii_case("mtl"));
        if selected != (selection == EntrySelection::Included) {
            return Err(invalid(
                "material filter differs from the physical MTL census",
            ));
        }
        let suffix = extension.unwrap_or(path);
        if suffix.eq_ignore_ascii_case("pak") || suffix.eq_ignore_ascii_case("socpak") {
            directory.pending_child = Some(ContainerIdentity::try_new_with_entry(archive, entry)?);
        }
        if !selected {
            return Ok(None);
        }
        self.counts.selected += 1;
        Ok(Some(Identity::Entry {
            containers: self.chain.clone(),
            entry: ContainerIdentity::try_new_with_entry(archive, entry)?,
        }))
    }

    pub(super) fn close(&mut self, chain: &[ContainerIdentity]) -> Result<(), MediaError> {
        let directory = self
            .directories
            .pop()
            .ok_or_else(|| invalid("material directory closed before opening"))?;
        if directory.pending_child.is_some() {
            return Err(invalid("material traversal skipped a physical container"));
        }
        if directory.next != directory.entries || self.chain != chain {
            return Err(invalid(
                "material traversal did not visit the complete physical directory",
            ));
        }
        self.root_complete = self.directories.is_empty();
        let _closed = self.chain.pop();
        Ok(())
    }

    pub(super) const fn counts(&self) -> &CensusCounts {
        &self.counts
    }

    pub(super) fn finish(self) -> Result<CensusCounts, MediaError> {
        if self.root_complete {
            Ok(self.counts)
        } else {
            Err(invalid(
                "material traversal did not complete the root directory",
            ))
        }
    }
}

fn invalid(message: &'static str) -> MediaError {
    MediaError::Contract(message.into())
}
