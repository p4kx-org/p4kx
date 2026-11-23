// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use p4kx_extraction::{Archive, CancelToken, Guid, PathKeys, SourceUse};
use rayon::prelude::*;

use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture};
use crate::entry_source::EntrySource;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::starmap_containers::instance::ContainerFile;
use crate::starmap_containers::jump_gate::JumpGate;
use crate::starmap_containers::location_declaration::ChildLocation;
use crate::starmap_containers::proof::ChildOrigins;
use crate::starmap_containers::unresolved::UnresolvedChild;
use crate::starmap_index::Index;
use crate::starmap_read::main_xml;
use crate::starmap_xml::AuthoredPose;
use crate::{CatalogError, Kind};
#[path = "starmap_children.rs"]
mod children;

mod descriptor_scope;
pub(crate) use descriptor_scope::DescriptorScope;

#[path = "starmap_descriptor_root.rs"]
mod descriptor_root;

#[path = "starmap_external_sources.rs"]
mod external_sources;
#[path = "starmap_failed_links.rs"]
mod failed_links;

#[path = "starmap_hcd.rs"]
pub(crate) mod hcd;

#[path = "starmap_entxml.rs"]
pub(crate) mod entxml;

const CHILDREN: &str = "ChildObjectContainers";
const CHILD: &str = "Child";

/// The position and rotation of a Child, or the clipped attribute texts of one that has none.
pub(crate) enum Pose {
    Valid(AuthoredPose),
    Malformed {
        pos: String,
        rot: String,
        entity: String,
    },
}

/// One Child element of a container.
pub(crate) struct Child {
    pub(crate) pose: Pose,
    pub(crate) record: Option<Guid>,
    /// The path of the external container the Child names.
    pub(crate) external: Option<AuthoredEntryPath>,
    pub(crate) unresolved_external: Option<Arc<UnresolvedChild>>,
    pub(crate) deferred_external: Option<Box<crate::starmap_containers::unresolved::DeferredChild>>,
    pub(crate) origins: Option<Arc<ChildOrigins>>,
    pub(crate) location: Option<Arc<ChildLocation>>,
    pub(crate) below: Level,
}

impl Child {
    pub(crate) fn proves_external_absence(
        &self,
        archive: &Archive,
        path: &AuthoredEntryPath,
        candidates: &[usize],
    ) -> bool {
        self.unresolved_external
            .as_ref()
            .is_some_and(|declaration| declaration.proves_missing_target(archive, path, candidates))
    }

    pub(crate) fn entity_name(&self) -> &str {
        self.origins
            .as_deref()
            .and_then(ChildOrigins::entity_name)
            .unwrap_or_default()
    }
}

/// The Children of one element.
#[derive(Default)]
pub(crate) struct Level {
    pub(crate) children: Vec<Child>,
    /// The list holds more Children than the budget of the parse allowed; the rest was not read.
    pub(crate) truncated: bool,
}

impl Drop for Level {
    fn drop(&mut self) {
        if self.children.is_empty() {
            return;
        }
        let mut children = mem::take(&mut self.children).into_iter();
        let mut parents = Vec::new();
        loop {
            if let Some(mut child) = children.next() {
                if !child.below.children.is_empty() {
                    let below = mem::take(&mut child.below.children);
                    if !children.as_slice().is_empty() {
                        parents.push(children);
                    }
                    children = below.into_iter();
                }
            } else if let Some(parent) = parents.pop() {
                children = parent;
            } else {
                return;
            }
        }
    }
}

/// What reading one container found.
pub(crate) enum Body {
    Unreadable(String),
    Unparsed,
    Parsed {
        root: Level,
        pending_external: bool,
        file: Box<ContainerFile>,
        fallback: Option<String>,
        gate: Option<Box<JumpGate>>,
        gate_proof: Option<crate::catalog_read::original::XmlNodeSource>,
    },
}

/// The shared Child count and read controls for the discovered containers.
struct Bounds<'a> {
    cancel: Option<&'a CancelToken>,
    children: Option<AtomicUsize>,
    scope: DescriptorScope,
    directory: Option<SourceUse>,
    location_grid_paths: OnceLock<PathKeys>,
}

impl Bounds<'_> {
    fn take(&self) -> bool {
        self.children.as_ref().is_none_or(|remaining| {
            remaining
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |left| {
                    left.checked_sub(1)
                })
                .is_ok()
        })
    }
}

struct DescriptorRead {
    body: Body,
    ledger: ExportLedger,
    name_inputs_complete: bool,
}

/// Reads and parses the container at `entry`.
fn parse(
    archive: &Archive,
    index: &Index<'_>,
    entry: usize,
    bounds: &Bounds<'_>,
    capture: &EntryCapture<'_>,
) -> Result<DescriptorRead, CatalogError> {
    let mut ledger = ExportLedger::default();
    let mut name_inputs_complete = false;
    let body = parse_body(
        archive,
        index,
        entry,
        bounds,
        capture,
        &mut ledger,
        &mut name_inputs_complete,
    )?;
    Ok(DescriptorRead {
        body,
        ledger,
        name_inputs_complete,
    })
}

fn parse_body(
    archive: &Archive,
    index: &Index<'_>,
    entry: usize,
    bounds: &Bounds<'_>,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    name_inputs_complete: &mut bool,
) -> Result<Body, CatalogError> {
    let Some(socpak) = index.entries.get(entry) else {
        ledger.incomplete_inventory(Kind::StarmapObjects);
        return Ok(Body::Unreadable("no such entry".to_owned()));
    };
    let main = match main_xml(archive, socpak, capture, ledger)? {
        Ok(main) => main,
        Err(reason) => {
            ledger.incomplete_inventory(Kind::StarmapObjects);
            return Ok(Body::Unreadable(reason));
        }
    };
    let files = entxml::EntXmlFiles {
        archive,
        outer: socpak,
        container: &main.container,
        capture,
        cancel: bounds.cancel,
    };
    let Ok(descriptor_root::ParsedRoot {
        mut root,
        file,
        entities,
        gate,
        gate_proof,
        pending_external,
        name_inputs_complete: observed_name_inputs,
    }) = descriptor_root::read(&main.bytes, bounds, &files, index, ledger)?
    else {
        ledger.incomplete_inventory(Kind::StarmapObjects);
        main.source.record(
            ledger,
            Kind::StarmapObjects,
            SourceDisposition::ProjectionFailed,
        );
        return Ok(Body::Unparsed);
    };
    let loss = children::source_loss(&mut root, &entities, capture.source_uses(), bounds.cancel)?;
    *name_inputs_complete = observed_name_inputs && !loss.projection;
    let grid_failed = bounds.scope.reads_location_physics()
        && matches!(
            file.location_physics,
            crate::location_physics::GridRead::Unavailable
        );
    record_projection(ledger, main.source, &loss, grid_failed);
    Ok(Body::Parsed {
        root,
        file: Box::new(file),
        pending_external,
        fallback: main.fallback,
        gate,
        gate_proof,
    })
}

fn record_projection(
    ledger: &mut ExportLedger,
    source: EntrySource,
    loss: &children::SourceLoss,
    grid_failed: bool,
) {
    if loss.inventory || grid_failed {
        ledger.incomplete_inventory(Kind::StarmapObjects);
    }
    let disposition = if loss.projection || grid_failed {
        SourceDisposition::ProjectionFailed
    } else {
        SourceDisposition::Projected
    };
    source.record(ledger, Kind::StarmapObjects, disposition);
}

/// The containers read, and how many parses that took.
pub(crate) struct Read {
    pub(crate) ledger: ExportLedger,
    pub(crate) name_inputs_complete: bool,
    pub(crate) bodies: HashMap<usize, Body>,
    #[cfg(test)]
    pub(crate) parsed: usize,
}

impl Read {
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
            self.name_inputs_complete &= descriptor.name_inputs_complete;
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
    let mut observed = Read {
        ledger: ExportLedger::default(),
        bodies: HashMap::new(),
        #[cfg(test)]
        parsed: 0,
        name_inputs_complete: true,
    };
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

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "starmap_parse/installed.rs"]
pub(crate) mod installed;
