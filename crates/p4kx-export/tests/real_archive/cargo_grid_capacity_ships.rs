// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::iter::Enumerate;
use std::slice::Iter;

use p4kx_assembly::{ArchiveFiles, Attachment, EntityNode, LoadoutResolver, PortNode};
use p4kx_extraction::{Archive, DataForge, Filter};
use serde_json::Value;

use super::original::{Grid, exact_record, legacy_scu, round};
use super::{TestResult, WireCatalog, index, same_scu, text};

pub(super) struct Ship {
    record: u32,
    path: String,
    total: Legacy,
    ports: BTreeSet<Occurrence>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Occurrence {
    port_ordinals: Vec<u32>,
    item: u32,
}

pub(super) fn select(
    archive: &Archive,
    forge: &DataForge<'_>,
    filter: &Filter,
    paths: &str,
    grids: &[Grid<'_>; 2],
) -> TestResult<Vec<Ship>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let files = ArchiveFiles::new(archive);
    let mut resolver = LoadoutResolver::new(forge)
        .with_files(&files)
        .stopping_at_docked_vehicles();
    let mut ids = BTreeSet::new();
    let mut ships = Vec::new();
    for path in paths.split(';') {
        let record = exact_record(forge, filter, path)?;
        if !ids.insert(record.index()) {
            return Err("a physical ship can only be nominated once".into());
        }
        let entity = resolver.resolve(&record)?;
        let ports = selected_ports(entity.ports(), grids)?;
        if ports.is_empty() {
            return Err(
                "the nominated ship must actually attach at least one selected cargo Grid".into(),
            );
        }
        ships.push(Ship {
            record: record.index(),
            path: path.to_owned(),
            ports,
            total: legacy_total(forge, &entity),
        });
    }
    Ok(ships)
}

struct Frame<'port> {
    ports: Enumerate<Iter<'port, PortNode>>,
    prefix_len: usize,
}

fn selected_ports(ports: &[PortNode], grids: &[Grid<'_>; 2]) -> TestResult<BTreeSet<Occurrence>> {
    let mut found = BTreeSet::new();
    let mut position = Vec::new();
    let mut stack = vec![Frame {
        ports: ports.iter().enumerate(),
        prefix_len: 0,
    }];
    while let Some(frame) = stack.last_mut() {
        let Some((ordinal, port)) = frame.ports.next() else {
            stack.pop();
            continue;
        };
        position.truncate(frame.prefix_len);
        let Attachment::Entity(entity) = port.attachment() else {
            continue;
        };
        if entity.is_docked_vehicle() {
            continue;
        }
        position.push(u32::try_from(ordinal)?);
        if let Some(item) = entity.record_index()
            && grids.iter().any(|grid| item == grid.item.record_index())
        {
            if !legacy_grid(entity) {
                return Err("the selected attachment must count in the legacy cargo total".into());
            }
            found.insert(Occurrence {
                port_ordinals: position.clone(),
                item,
            });
        }
        if !port.nested().is_empty() {
            stack.push(Frame {
                ports: port.nested().iter().enumerate(),
                prefix_len: position.len(),
            });
        }
    }
    Ok(found)
}

pub(crate) struct Legacy {
    pub(crate) scu: Option<f64>,
    pub(crate) issues: u64,
}

pub(crate) fn legacy_total(forge: &DataForge<'_>, ship: &EntityNode) -> Legacy {
    let mut sum = 0.0;
    let mut known = false;
    let mut issues = 0;
    for port in ship.loadout() {
        let Some(item) = port.entity().filter(|node| legacy_grid(node)) else {
            continue;
        };
        let capacity = item
            .inventory_container()
            .and_then(|guid| forge.record_by_guid(guid))
            .and_then(|record| record.tree().ok())
            .and_then(|container| legacy_scu(container.root()));
        match capacity {
            Some(scu) => {
                sum += scu;
                known = true;
            }
            None => issues += 1,
        }
    }
    Legacy {
        scu: (known || issues == 0).then(|| round(sum)),
        issues,
    }
}

pub(crate) fn legacy_grid(node: &EntityNode) -> bool {
    node.attach_type().eq_ignore_ascii_case("CargoGrid")
        && !node
            .class_name()
            .to_ascii_lowercase()
            .ends_with("_template")
        && !node
            .record_path()
            .to_ascii_lowercase()
            .ends_with("_cargogrid_template")
}

#[derive(Default)]
pub(super) struct Counts {
    pub(super) placements: usize,
    pub(super) repeats: usize,
    pub(super) ships: usize,
}

pub(super) fn compare(
    catalog: &WireCatalog,
    ships: &[Ship],
    grids: &[Grid<'_>; 2],
) -> TestResult<Counts> {
    let mut counts = Counts::default();
    for ship in ships {
        compare_total(&catalog.ships, ship)?;
        let placements = &catalog.placements.facility_instances.rows;
        let actual = published_ports(placements, ship, grids)?;
        if actual != ship.ports {
            return Err(
                "the placed cargo Grids must match the original physical Loadout ports".into(),
            );
        }
        let distinct: BTreeSet<_> = actual.iter().map(|position| position.item).collect();
        counts.placements += actual.len();
        counts.repeats += actual.len() - distinct.len();
        counts.ships += 1;
    }
    Ok(counts)
}

fn compare_total(rows: &[Value], ship: &Ship) -> TestResult {
    let mut found = rows
        .iter()
        .filter(|row| text(row, "/recordPath") == Some(ship.path.as_str()));
    let row = found
        .next()
        .ok_or("the nominated physical Ship must be present")?;
    let cargo = row.get("cargoScu").ok_or("legacy cargoScu field")?;
    if found.next().is_some()
        || !(cargo.is_null() || cargo.is_number())
        || !same_scu(cargo.as_f64(), ship.total.scu)
        || index(row, "/cargoGridsWithIssue") != Some(ship.total.issues)
    {
        return Err(
            "the legacy cargo sum and unresolved grid count differ from original Loadout operands"
                .into(),
        );
    }
    Ok(())
}

fn published_ports(
    rows: &[Value],
    ship: &Ship,
    grids: &[Grid<'_>; 2],
) -> TestResult<BTreeSet<Occurrence>> {
    let mut positions = BTreeSet::new();
    for row in rows.iter().filter(|row| {
        index(row, "/shipRecordIndex/value") == Some(u64::from(ship.record))
            && text(row, "/source/kind") == Some("loadout")
    }) {
        let attached = index(row, "/source/attachedItem/targetRecordIndex/value");
        let Some(grid) = grids
            .iter()
            .find(|grid| attached == Some(u64::from(grid.item.record_index())))
        else {
            continue;
        };
        let links = row
            .get("configurations")
            .and_then(Value::as_array)
            .ok_or("placed configuration links")?;
        let mut matching = links.iter().filter(|link| {
            text(link, "/kind") == Some("facility")
                && index(link, "/owner/ownerRecordIndex/value")
                    == Some(u64::from(grid.item.record_index()))
                && index(link, "/owner/componentOrdinal/value")
                    == u64::try_from(grid.facility_ordinal).ok()
        });
        if matching.next().is_none() || matching.next().is_some() {
            return Err(
                "the cargo placement must link its exact physical configuration once".into(),
            );
        }
        let ordinals = row
            .pointer("/source/position/value/portOrdinals")
            .and_then(Value::as_array)
            .ok_or("physical port ordinals")?;
        let port_ordinals = ordinals
            .iter()
            .map(|ordinal| {
                Ok(u32::try_from(
                    ordinal.as_u64().ok_or("physical port position")?,
                )?)
            })
            .collect::<TestResult<Vec<_>>>()?;
        if !positions.insert(Occurrence {
            port_ordinals,
            item: grid.item.record_index(),
        }) {
            return Err("the same physical cargo placement was published twice".into());
        }
    }
    Ok(positions)
}
