// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::Archive;

use super::{Body, Child, Level};
use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture};
use crate::entry_source::{self, EntrySource};
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::starmap_containers::name_coverage;
use crate::starmap_index::Index;
use crate::{CatalogError, Kind};

pub(super) struct ExternalSources {
    pub(super) next: HashSet<usize>,
    pub(super) name_inputs_complete: bool,
}

pub(super) fn next_sources<'a>(
    bodies: impl Iterator<Item = &'a Body>,
    index: &Index<'_>,
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
) -> Result<ExternalSources, CatalogError> {
    let mut next = HashSet::new();
    let mut observations = ExportLedger::default();
    let mut paths = Vec::new();
    for body in bodies {
        if let Body::Parsed { root, .. } = body {
            paths.clear();
            externals(root, &mut paths);
            for child in &paths {
                let Some(path) = child.external.as_ref() else {
                    continue;
                };
                let candidates = index.candidates(path.as_str());
                match candidates {
                    [entry] => {
                        next.insert(*entry);
                    }
                    [] => {
                        let disposition =
                            if child.proves_external_absence(archive, path, candidates) {
                                SourceDisposition::VerifiedMissingReference
                            } else {
                                SourceDisposition::ReadFailed
                            };
                        EntrySource::authored_missing(AuthoredEntryPath::clone(path))
                            .dependency()
                            .record(&mut observations, Kind::StarmapObjects, disposition);
                    }
                    candidates => entry_source::record_ambiguous_outer_entries(
                        archive,
                        capture,
                        &mut observations,
                        Kind::StarmapObjects,
                        Some(path),
                        candidates,
                    )?,
                }
            }
        }
    }
    let name_inputs_complete = name_coverage::descriptor_inputs_complete(&observations);
    ledger.merge(observations);
    Ok(ExternalSources {
        next,
        name_inputs_complete,
    })
}

fn externals<'level>(level: &'level Level, out: &mut Vec<&'level Child>) {
    let mut parents = Vec::new();
    let mut children = level.children.iter();
    loop {
        if let Some(child) = children.next() {
            if child.external.is_some() {
                out.push(child);
            }
            if !child.below.children.is_empty() {
                parents.push(children);
                children = child.below.children.iter();
            }
        } else if let Some(parent) = parents.pop() {
            children = parent;
        } else {
            return;
        }
    }
}
