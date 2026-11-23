// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::sync::atomic::AtomicUsize;

use p4kx_extraction::{Archive, CancelToken, SourceUse};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};

use super::{
    Body, Bounds, DescriptorRead, DescriptorScope, DescriptorSurvey, external_sources,
    failed_links, parse,
};
use crate::CatalogError;
use crate::catalog_read::original::EntryCapture;
use crate::export_ledger::ExportLedger;
use crate::starmap_index::Index;

/// The containers read, and how many parses that took.
pub(crate) struct Read {
    pub(crate) ledger: ExportLedger,
    pub(crate) name_inputs_complete: bool,
    pub(crate) bodies: HashMap<usize, Body>,
    pub(crate) survey: Option<DescriptorSurvey>,
    #[cfg(test)]
    pub(crate) parsed: usize,
}

impl Read {
    fn new(scope: DescriptorScope, directory: Option<&SourceUse>) -> Self {
        Self {
            ledger: ExportLedger::default(),
            bodies: HashMap::new(),
            #[cfg(test)]
            parsed: 0,
            name_inputs_complete: true,
            survey: scope
                .captures_loot_survey()
                .then(|| DescriptorSurvey::new(directory.cloned())),
        }
    }

    fn retain_frontier(
        &mut self,
        descriptors: Vec<(usize, DescriptorRead)>,
        scope: DescriptorScope,
        pending: &mut HashSet<usize>,
    ) {
        #[cfg(test)]
        {
            self.parsed += descriptors.len();
        }
        for (entry, descriptor) in descriptors {
            if scope.resolves_placements() {
                failed_links::observe(entry, &descriptor.body, pending);
            }
            self.name_inputs_complete &= descriptor.observations.name_inputs_complete;
            if let (Some(survey), Some(inputs)) = (&mut self.survey, descriptor.observations.survey)
            {
                survey.observe(inputs);
            }
            self.bodies.insert(entry, descriptor.body);
            self.ledger.merge(descriptor.ledger);
        }
    }
}

/// Reads each physically distinct container reachable from the roots in breadth-first order.
#[cfg(test)]
pub(crate) fn read_all(
    archive: &Archive,
    index: &Index<'_>,
    roots: &[usize],
    cancel: Option<&CancelToken>,
    children: Option<usize>,
    capture: &EntryCapture<'_>,
) -> Result<Read, CatalogError> {
    read_all_scoped(
        archive,
        index,
        roots,
        cancel,
        (
            children,
            DescriptorScope::Catalog {
                jump_endpoints: false,
            },
        ),
        capture,
    )
}

pub(crate) fn read_all_scoped(
    archive: &Archive,
    index: &Index<'_>,
    roots: &[usize],
    cancel: Option<&CancelToken>,
    (children, scope): (Option<usize>, DescriptorScope),
    capture: &EntryCapture<'_>,
) -> Result<Read, CatalogError> {
    let bounds = Bounds {
        cancel,
        children: children.map(AtomicUsize::new),
        location_grid_paths: OnceLock::new(),
        scope,
        directory: capture
            .source_uses()
            .observe_root_directory_paths(archive)?,
    };
    let mut observed = Read::new(scope, bounds.directory.as_ref());
    let mut pending = HashSet::new();
    let mut frontier: Vec<usize> = roots
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    while !frontier.is_empty() && !cancel.is_some_and(CancelToken::is_cancelled) {
        let read: Vec<_> = frontier
            .par_iter()
            .filter(|_| !cancel.is_some_and(CancelToken::is_cancelled))
            .map(|&entry| parse(archive, index, entry, &bounds, capture).map(|read| (entry, read)))
            .collect::<Result<_, _>>()?;
        let external = external_sources::next_sources(
            read.iter().map(|(_, descriptor)| &descriptor.body),
            index,
            archive,
            capture,
            &mut observed.ledger,
        )?;
        observed.name_inputs_complete &= external.name_inputs_complete;
        observed.retain_frontier(read, scope, &mut pending);
        if scope.resolves_placements() {
            failed_links::finish(&mut observed.bodies, &mut pending, index, capture, cancel)?;
        }
        frontier = external
            .next
            .into_iter()
            .filter(|entry| !observed.bodies.contains_key(entry))
            .collect();
    }
    if scope.resolves_placements() {
        failed_links::completed(&pending, cancel)?;
    }
    Ok(observed)
}
