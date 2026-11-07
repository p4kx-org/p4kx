// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{CancelToken, DataForge, TagTree};
use rayon::prelude::*;

use crate::contract::{Item, Paint};
use crate::guid_text::CatalogGuid;
use crate::paint_port::{PaintPort, VehicleFacts};
use crate::{CatalogError, PaintSummary};

const PAINT_TYPE: &str = "Paints";

/// Returns whether `kind` is the `AttachDef.Type` of a paint, ignoring ASCII case.
pub(crate) const fn is_paint_type(kind: &str) -> bool {
    kind.eq_ignore_ascii_case(PAINT_TYPE)
}

/// The paints of a catalog and what was counted deriving them.
pub(crate) struct Paints {
    pub(crate) ledger: crate::export_ledger::ExportLedger,
    pub(crate) failures: Vec<crate::CatalogFailure>,
    pub(crate) list: Vec<Paint>,
    pub(crate) summary: PaintSummary,
    pub(crate) unreadable_files: Vec<String>,
}

/// Returns the paints among `items` with the ships of `vehicles` they fit.
///
/// A paint is an item whose `AttachDef.Type` is `Paints`. It fits a ship when a paint port of the
/// ship accepts it: every tag in the port's required tags is among the paint's `Tags` or above
/// one of them in the tag tree, and every tag in the paint's own `RequiredTags` is among the
/// port's tags and the ship Record's `AttachDef.Tags` or above one of them. The ports are the
/// `SItemPortDef` entries of the ship's Record and the `ItemPort` elements of its vehicle
/// implementation.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
pub(crate) fn paints(
    forge: &DataForge<'_>,
    items: &[Item<'_>],
    vehicles: &[VehicleFacts],
    sources: &crate::export_ledger::ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<Paints, CatalogError> {
    let candidates: Vec<&Item<'_>> = items
        .iter()
        .filter(|item| item.r#type.is_some_and(is_paint_type))
        .collect();
    let mut ledger = crate::paint_ledger::inputs(sources, &candidates, vehicles)?;
    if candidates.is_empty() {
        return Ok(Paints {
            ledger,
            failures: Vec::new(),
            list: Vec::new(),
            summary: PaintSummary::default(),
            unreadable_files: Vec::new(),
        });
    }
    let (groups, mut unreadable_files) = group_ships(vehicles);
    let unreadable = unreadable_files.len();
    unreadable_files.sort_unstable();
    unreadable_files.dedup();
    let tags = crate::export_tag_read::read(forge, crate::Kind::Paints, cancel)?;
    ledger.merge(tags.ledger);
    let index = TagIndex::new(&tags.tree);
    let mut paints: Vec<Paint> = candidates
        .par_iter()
        .map(|item| {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            let have = lowered(&item.tags);
            let required = lowered(&item.required_tags);
            let mut ship_guids: Vec<CatalogGuid> = groups
                .iter()
                .filter(|(group, _)| group.accepts(&have, &required, &index))
                .flat_map(|(_, guids)| guids.iter().copied())
                .collect();
            ship_guids.sort_by_cached_key(|guid| guid.text());
            Ok(Paint {
                item_guid: item.guid,
                ship_guids,
            })
        })
        .collect::<Result<_, _>>()?;
    paints.sort_by_cached_key(|paint| paint.item_guid.text());
    let summary = summarise(&paints, unreadable);
    Ok(Paints {
        ledger,
        failures: tags.failures,
        list: paints,
        summary,
        unreadable_files,
    })
}

/// What ships share when it comes to the paints they accept: their paint ports and the tags of
/// their Record.
#[derive(Debug, PartialEq, Eq, Hash)]
struct ShipPorts {
    ports: Vec<PaintPort>,
    tags: Vec<String>,
}

impl ShipPorts {
    fn accepts(&self, have: &[String], required: &[String], index: &TagIndex<'_>) -> bool {
        self.ports.iter().any(|port| {
            fits(port, have, index)
                && required.iter().all(|wanted| {
                    port.port_tags
                        .iter()
                        .chain(&port.required_tags)
                        .chain(&self.tags)
                        .any(|offered| index.matches(offered, wanted))
                })
        })
    }
}

type ShipsByPorts = HashMap<ShipPorts, Vec<CatalogGuid>>;

/// Groups the ships by their paint ports, those of their Record and those of their vehicle
/// implementation, and the tags of their Record, and returns why the files of the ships that could
/// not be used cannot be read, one entry per ship.
fn group_ships(vehicles: &[VehicleFacts]) -> (ShipsByPorts, Vec<String>) {
    let mut by_ports = ShipsByPorts::new();
    let mut unreadable = Vec::new();
    for vehicle in vehicles {
        let mut ports = vehicle.paint_ports.clone();
        unreadable.extend(vehicle.implementation.unreadable.iter().cloned());
        ports.extend(vehicle.implementation.ports.iter().cloned());
        ports.sort_unstable();
        ports.dedup();
        if !ports.is_empty() {
            let mut tags = vehicle.tags.clone();
            tags.sort_unstable();
            tags.dedup();
            by_ports
                .entry(ShipPorts { ports, tags })
                .or_default()
                .push(vehicle.ship);
        }
    }
    (by_ports, unreadable)
}

fn summarise(paints: &[Paint], unreadable_vehicles: usize) -> PaintSummary {
    let with_paints: HashSet<[u8; 36]> = paints
        .iter()
        .flat_map(|paint| paint.ship_guids.iter().map(|guid| guid.text()))
        .collect();
    let assigned = paints
        .iter()
        .filter(|paint| !paint.ship_guids.is_empty())
        .count();
    PaintSummary {
        paints: paints.len(),
        assigned,
        unassigned: paints.len().saturating_sub(assigned),
        ships: with_paints.len(),
        unreadable_vehicles,
    }
}

fn lowered(tags: &[&str]) -> Vec<String> {
    tags.iter().map(|tag| tag.to_ascii_lowercase()).collect()
}

fn fits(port: &PaintPort, have: &[String], index: &TagIndex<'_>) -> bool {
    !port.required_tags.is_empty()
        && port
            .required_tags
            .iter()
            .all(|wanted| have.iter().any(|tag| index.matches(tag, wanted)))
}

/// The tags of the tag tree by lower-case name.
struct TagIndex<'t> {
    tree: &'t TagTree,
    positions: HashMap<String, Vec<usize>>,
}

impl<'t> TagIndex<'t> {
    fn new(tree: &'t TagTree) -> Self {
        let mut positions: HashMap<String, Vec<usize>> = HashMap::new();
        for tag in tree.iter() {
            positions
                .entry(tag.name().to_ascii_lowercase())
                .or_default()
                .push(tag.position());
        }
        Self { tree, positions }
    }

    /// Returns whether the tag `have` is the tag `wanted` or lies below a tag of that name.
    fn matches(&self, have: &str, wanted: &str) -> bool {
        if have == wanted {
            return true;
        }
        let (Some(below), Some(above)) = (self.positions.get(have), self.positions.get(wanted))
        else {
            return false;
        };
        below.iter().filter_map(|&at| self.tree.at(at)).any(|tag| {
            above
                .iter()
                .filter_map(|&at| self.tree.at(at))
                .any(|ancestor| tag.is_within(&ancestor))
        })
    }
}
