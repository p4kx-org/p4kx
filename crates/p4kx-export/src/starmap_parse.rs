// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_extraction::cryxml::{self, Xml};
use p4kx_extraction::{Archive, CancelToken, Guid};
use rayon::prelude::*;

use crate::Kind;
use crate::entry_source::EntrySource;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::starmap_index::Index;
use crate::starmap_read::main_xml;
use crate::starmap_report::clip;
use crate::starmap_transform::Rotation;
use crate::starmap_xml::{Node, attribute, first_below, pose};

const CHILDREN: &str = "ChildObjectContainers";
const CHILD: &str = "Child";

/// The position and rotation of a Child, or the clipped attribute texts of one that has none.
pub(crate) enum Pose {
    Valid {
        position: [f64; 3],
        rotation: Rotation,
    },
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
    pub(crate) external: Option<String>,
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

/// Returns the Children of `node`; `depth` is the number of lists above it.
fn level<'d, N: Node<'d>>(node: N, depth: usize, bounds: &Bounds) -> Level {
    let Some(list) = first_below(node, CHILDREN) else {
        return Level::default();
    };
    let extra_lists = node
        .children()
        .filter(|sibling| sibling.name().eq_ignore_ascii_case(CHILDREN))
        .nth(1)
        .is_some();
    if depth >= bounds.depth {
        return Level {
            capped: list
                .children()
                .any(|child| child.name().eq_ignore_ascii_case(CHILD)),
            extra_lists,
            ..Level::default()
        };
    }
    let mut children = Vec::new();
    let mut truncated = false;
    for child in list
        .children()
        .filter(|child| child.name().eq_ignore_ascii_case(CHILD))
    {
        if !bounds.take() {
            truncated = true;
            break;
        }
        children.push(read_child(child, depth, bounds));
    }
    Level {
        children,
        capped: false,
        extra_lists,
        truncated,
    }
}

fn read_child<'d, N: Node<'d>>(child: N, depth: usize, bounds: &Bounds) -> Child {
    let record = attribute(child, "starMapRecord")
        .and_then(|text| text.trim().parse::<Guid>().ok())
        .filter(|guid| guid.stored() != [0; 16]);
    let external = (attribute(child, "external") == Some("1"))
        .then(|| attribute(child, "name").filter(|path| !path.is_empty()))
        .flatten()
        .map(str::to_owned);
    let pose = match pose(child) {
        Some((position, rotation)) => Pose::Valid { position, rotation },
        None => Pose::Malformed {
            pos: clip(attribute(child, "pos").unwrap_or_default()),
            rot: clip(attribute(child, "rot").unwrap_or_default()),
            entity: clip(attribute(child, "entityName").unwrap_or_default()),
        },
    };
    let below = level(child, depth + 1, bounds);
    let named = record.is_some() || external.is_some() || !below.children.is_empty();
    Child {
        pose,
        entity_name: if named {
            attribute(child, "entityName")
                .unwrap_or_default()
                .to_owned()
        } else {
            String::new()
        },
        record,
        external,
        below,
    }
}

/// Reads and parses the container at `entry`.
fn parse(
    archive: &Archive,
    index: &Index<'_>,
    entry: usize,
    bounds: &Bounds,
) -> (Body, ExportLedger) {
    let mut ledger = ExportLedger::default();
    let body = parse_body(archive, index, entry, bounds, &mut ledger);
    (body, ledger)
}

fn parse_body(
    archive: &Archive,
    index: &Index<'_>,
    entry: usize,
    bounds: &Bounds,
    ledger: &mut ExportLedger,
) -> Body {
    let Some(socpak) = index.entries.get(entry) else {
        ledger.incomplete_inventory(Kind::StarmapObjects);
        return Body::Unreadable("no such entry".to_owned());
    };
    let main = match main_xml(archive, socpak, ledger) {
        Ok(main) => main,
        Err(reason) => {
            ledger.incomplete_inventory(Kind::StarmapObjects);
            return Body::Unreadable(reason);
        }
    };
    let Some(root) = parsed_root(&main.bytes, bounds) else {
        ledger.incomplete_inventory(Kind::StarmapObjects);
        main.source.record(
            ledger,
            Kind::StarmapObjects,
            SourceDisposition::ProjectionFailed,
        );
        return Body::Unparsed;
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
    Body::Parsed {
        root,
        fallback: main.fallback,
    }
}

fn parsed_root(bytes: &[u8], bounds: &Bounds) -> Option<Level> {
    match cryxml::read(bytes) {
        Ok(Xml::CryXml(document)) => Some(level(document.root(), 0, bounds)),
        Ok(Xml::Text(text)) => {
            let parsed = std::str::from_utf8(text)
                .ok()
                .map(|text| text.trim_start_matches('\u{feff}'))
                .filter(|text| cryxml::check_text_depth(text).is_ok())
                .and_then(|text| roxmltree::Document::parse(text).ok());
            parsed.map(|document| level(document.root_element(), 0, bounds))
        }
        Err(_) => None,
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

fn externals<'l>(level: &'l Level, out: &mut Vec<&'l str>) {
    for child in &level.children {
        out.extend(child.external.as_deref());
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
) -> Read {
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
            .map(|&entry| (entry, parse(archive, index, entry, &bounds)))
            .collect();
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
        record_unread(archive, &frontier, &mut ledger);
    }
    Read {
        ledger,
        bodies,
        parsed,
    }
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
                if let Some(entry) = index.find(path) {
                    next.insert(entry);
                } else {
                    EntrySource::missing(path).dependency().record(
                        ledger,
                        Kind::StarmapObjects,
                        SourceDisposition::ReadFailed,
                    );
                }
            }
        }
    }
    next
}

fn record_unread(archive: &Archive, entries: &[usize], ledger: &mut ExportLedger) {
    if !entries.is_empty() {
        ledger.incomplete_inventory(Kind::StarmapObjects);
    }
    for index in entries {
        if let Some(entry) = archive.entries().get(*index) {
            EntrySource::outer(entry)
                .dependency()
                .operation(EntryOperation::Enumeration)
                .record(ledger, Kind::StarmapObjects, SourceDisposition::ReadFailed);
        }
    }
}

#[cfg(test)]
mod tests;
