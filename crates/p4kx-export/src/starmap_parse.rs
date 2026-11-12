// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_extraction::{Archive, CancelToken, Guid};
use rayon::prelude::*;

use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, EntryRead, EntryXml};
use crate::entry_source::EntrySource;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::starmap_containers::instance::ContainerFile;
use crate::starmap_containers::proof::ChildOrigins;
use crate::starmap_index::Index;
use crate::starmap_read::main_xml;
use crate::starmap_xml::{AuthoredPose, XmlError};
use crate::{CatalogError, Kind};
#[path = "starmap_children.rs"]
mod children;

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
    /// The `entityName`, kept for a Child that names a record or an external container or has
    /// Children of its own.
    pub(crate) entity_name: String,
    /// The path of the external container the Child names.
    pub(crate) external: Option<AuthoredEntryPath>,
    pub(crate) origins: Option<Arc<ChildOrigins>>,
    pub(crate) below: Level,
}

/// The Children of one element.
#[derive(Default)]
pub(crate) struct Level {
    pub(crate) children: Vec<Child>,
    /// The list lies beyond the depth limit and was not read.
    pub(crate) capped: bool,
    /// The element holds more than one `ChildObjectContainers`; only the first was read.
    pub(crate) extra_lists: bool,
    /// The list holds more Children than the budget of the parse allowed; the rest was not read.
    pub(crate) truncated: bool,
}

/// What reading one container found.
pub(crate) enum Body {
    Unreadable(String),
    Unparsed,
    Parsed {
        root: Level,
        file: Box<ContainerFile>,
        fallback: Option<String>,
    },
}

/// What bounds the parse: the nesting depth and the number of Children kept in all containers.
struct Bounds {
    depth: usize,
    children: AtomicUsize,
}

impl Bounds {
    fn take(&self) -> bool {
        self.children
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |left| {
                left.checked_sub(1)
            })
            .is_ok()
    }
}

/// Reads and parses the container at `entry`.
fn parse(
    archive: &Archive,
    index: &Index<'_>,
    entry: usize,
    bounds: &Bounds,
    capture: &EntryCapture<'_>,
) -> Result<(Body, ExportLedger), CatalogError> {
    let mut ledger = ExportLedger::default();
    let body = parse_body(archive, index, entry, bounds, capture, &mut ledger)?;
    Ok((body, ledger))
}

fn parse_body(
    archive: &Archive,
    index: &Index<'_>,
    entry: usize,
    bounds: &Bounds,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
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
    let Ok((root, file)) = parsed_root(&main.bytes, bounds, capture)? else {
        ledger.incomplete_inventory(Kind::StarmapObjects);
        main.source.record(
            ledger,
            Kind::StarmapObjects,
            SourceDisposition::ProjectionFailed,
        );
        return Ok(Body::Unparsed);
    };
    let loss = source_loss(&root);
    if loss.inventory {
        ledger.incomplete_inventory(Kind::StarmapObjects);
    }
    let disposition = if loss.projection {
        SourceDisposition::ProjectionFailed
    } else {
        SourceDisposition::Projected
    };
    main.source
        .record(ledger, Kind::StarmapObjects, disposition);
    Ok(Body::Parsed {
        root,
        file: Box::new(file),
        fallback: main.fallback,
    })
}

fn parsed_root(
    read: &EntryRead,
    bounds: &Bounds,
    capture: &EntryCapture<'_>,
) -> Result<Result<(Level, ContainerFile), XmlError>, CatalogError> {
    match EntryXml::parse(read, capture)? {
        Ok(xml) => {
            let root = xml.root();
            Ok(Ok((
                children::level(root, 0, bounds, &xml)?,
                ContainerFile::from_xml(&xml, capture)?,
            )))
        }
        Err(error) => Ok(Err(error)),
    }
}
struct SourceLoss {
    projection: bool,
    inventory: bool,
}

fn source_loss(level: &Level) -> SourceLoss {
    let inventory = level.capped || level.truncated || level.extra_lists;
    let mut loss = SourceLoss {
        projection: inventory,
        inventory,
    };
    for child in &level.children {
        let below = source_loss(&child.below);
        loss.projection |= matches!(&child.pose, Pose::Malformed { .. }) || below.projection;
        loss.inventory |= below.inventory;
    }
    loss
}

fn externals<'level>(level: &'level Level, out: &mut Vec<&'level AuthoredEntryPath>) {
    for child in &level.children {
        out.extend(child.external.as_ref());
        externals(&child.below, out);
    }
}
/// The containers read, and how many parses that took.
pub(crate) struct Read {
    pub(crate) ledger: ExportLedger,
    pub(crate) bodies: HashMap<usize, Body>,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "only the tests count the parses")
    )]
    pub(crate) parsed: usize,
}

/// Reads every container that the containers `roots` name, directly or through other containers
/// within `chain` steps, once each, the containers of one step in parallel. Together the
/// containers keep at most `children` Children and nest their lists at most `depth` deep.
pub(crate) fn read_all(
    archive: &Archive,
    index: &Index<'_>,
    roots: &[usize],
    cancel: Option<&CancelToken>,
    chain: usize,
    (depth, children): (usize, usize),
    capture: &EntryCapture<'_>,
) -> Result<Read, CatalogError> {
    let bounds = Bounds {
        depth,
        children: AtomicUsize::new(children),
    };
    let mut bodies: HashMap<usize, Body> = HashMap::new();
    let mut ledger = ExportLedger::default();
    let mut parsed = 0;
    let mut frontier: Vec<usize> = roots
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    for _ in 0..=chain {
        if frontier.is_empty() || cancel.is_some_and(CancelToken::is_cancelled) {
            break;
        }
        let read: Vec<_> = frontier
            .par_iter()
            .filter(|_| !cancel.is_some_and(CancelToken::is_cancelled))
            .map(|&entry| parse(archive, index, entry, &bounds, capture).map(|read| (entry, read)))
            .collect::<Result<_, _>>()?;
        parsed += read.len();
        let next = next_sources(read.iter().map(|(_, (body, _))| body), index, &mut ledger);
        for (entry, (body, observed)) in read {
            bodies.insert(entry, body);
            ledger.merge(observed);
        }
        frontier = next
            .into_iter()
            .filter(|entry| !bodies.contains_key(entry))
            .collect();
    }
    if !cancel.is_some_and(CancelToken::is_cancelled) {
        record_unread(archive, &frontier, capture, &mut ledger)?;
    }
    Ok(Read {
        ledger,
        bodies,
        parsed,
    })
}

fn next_sources<'a>(
    bodies: impl Iterator<Item = &'a Body>,
    index: &Index<'_>,
    ledger: &mut ExportLedger,
) -> HashSet<usize> {
    let mut next = HashSet::new();
    let mut paths = Vec::new();
    for body in bodies {
        if let Body::Parsed { root, .. } = body {
            paths.clear();
            externals(root, &mut paths);
            for path in &paths {
                if let Some(entry) = index.find(path.as_str()) {
                    next.insert(entry);
                } else {
                    EntrySource::authored_missing(AuthoredEntryPath::clone(path))
                        .dependency()
                        .record(ledger, Kind::StarmapObjects, SourceDisposition::ReadFailed);
                }
            }
        }
    }
    next
}

fn record_unread(
    archive: &Archive,
    entries: &[usize],
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
) -> Result<(), CatalogError> {
    if !entries.is_empty() {
        ledger.incomplete_inventory(Kind::StarmapObjects);
    }
    for index in entries {
        if let Some(entry) = archive.entries().get(*index) {
            EntrySource::outer(capture, archive, entry)?
                .dependency()
                .operation(EntryOperation::Enumeration)
                .record(ledger, Kind::StarmapObjects, SourceDisposition::ReadFailed);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
