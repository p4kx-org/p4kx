// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Entry;

use crate::{CandidateInventory, CandidateState, MediaError, MediaKind, MediaWriter};

use super::{DirectoryLookup, LookupWork, directory_index};

impl DirectoryLookup<'_> {
    pub(crate) fn visit_path_candidates(
        &self,
        path: &str,
        kind: Option<MediaKind>,
        writer: &mut MediaWriter<'_>,
        mut visit: impl FnMut(&Entry, CandidateState, &mut MediaWriter<'_>) -> Result<(), MediaError>,
    ) -> Result<CandidateInventory, MediaError> {
        writer.start_reference()?;
        let work = LookupWork {
            checkpoint: writer.checkpoint,
            limits: writer.limits(),
            current: self.work,
            source_allowance: self.allowance(writer.limits().sources),
        };
        let paths = self.index.index(&work)?;
        let requested = kind.is_none_or(|kind| writer.has_kind(kind));
        let entries = paths
            .matching_entries_with(path, |count| work.charge(count))
            .map_err(directory_index::error)?;
        for entry in entries {
            let entry = entry.map_err(directory_index::error)?;
            visit(entry, self.candidate_state(entry, requested), writer)?;
        }
        work.charge(0)?;
        Ok(self.index.inventory())
    }
}
