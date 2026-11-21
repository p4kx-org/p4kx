// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_assembly::PaintTagIndex;
use p4kx_extraction::CancelToken;
use rayon::prelude::*;

use crate::catalog_v3::tag_catalog::TagDependencies;
use crate::contract::{Item, Paint};
use crate::guid_text::CatalogGuid;
use crate::paint_port::{PaintPort, VehicleFacts};
use crate::{CatalogError, ExportAccountingError, PaintSummary};

const PAINT_TYPE: &str = "Paints";

/// Returns whether `kind` is the `AttachDef.Type` of a paint, ignoring ASCII case.
pub(crate) const fn is_paint_type(kind: &str) -> bool {
    kind.eq_ignore_ascii_case(PAINT_TYPE)
}

/// Returns whether the selected item is a paint.
pub(crate) fn is_paint(item: &Item<'_>) -> bool {
    item.paint.item_type.is_some_and(is_paint_type)
}

/// The paints of a catalog and what was counted deriving them.
pub(crate) struct Paints<'f> {
    pub(crate) ledger: crate::export_ledger::ExportLedger,
    pub(crate) failures: Vec<crate::CatalogReadFailure<'f>>,
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
pub(crate) fn paints<'f>(
    tags: Option<TagDependencies<'f>>,
    items: &[Item<'_>],
    vehicles: &[VehicleFacts],
    sources: &crate::export_ledger::ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<Paints<'f>, CatalogError> {
    let candidates: Vec<&Item<'_>> = items.iter().filter(|item| is_paint(item)).collect();
    let mut ledger = crate::paint_ledger::inputs(sources, &candidates, vehicles)?;
    if candidates.is_empty() != tags.is_none() {
        return Err(ExportAccountingError::ConflictingTagTrace.into());
    }
    let Some(tags) = tags else {
        return Ok(Paints {
            ledger,
            failures: Vec::new(),
            list: Vec::new(),
            summary: PaintSummary::default(),
            unreadable_files: Vec::new(),
        });
    };
    let (groups, mut unreadable_files) = group_ships(vehicles);
    let unreadable = unreadable_files.len();
    unreadable_files.sort_unstable();
    unreadable_files.dedup();
    ledger.merge(tags.ledger);
    let index = PaintTagIndex::new(tags.tree.tree());
    let mut paints: Vec<Paint> = candidates
        .par_iter()
        .map(|item| {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            let have = lowered(&item.paint.tags);
            let required = lowered(&item.paint.required_tags);
            let mut ship_guids: Vec<CatalogGuid> = groups
                .iter()
                .filter(|(group, _)| group.accepts(&have, &required, &index))
                .flat_map(|(_, guids)| guids.iter().copied())
                .collect();
            ship_guids.sort_by_cached_key(|guid| guid.text());
            Ok(Paint {
                item_guid: item.identity.guid,
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
    fn accepts(&self, have: &[String], required: &[String], index: &PaintTagIndex<'_>) -> bool {
        self.ports
            .iter()
            .any(|port| port.accepts(have, required, &self.tags, index))
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
