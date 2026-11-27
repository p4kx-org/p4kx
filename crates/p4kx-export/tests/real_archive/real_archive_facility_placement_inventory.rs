// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{DataForge, Guid, Record, TreeBuilder, Value};

use super::Result;

const FACILITIES: &[&str] = &[
    "SCItemSeatParams",
    "MedBedComponentParams",
    "SCItemCargoGridParams",
    "SCItemInventoryContainerComponentParams",
    "LogoutComponentParams",
    "SEntityComponentOutfitHangerParams",
];

#[derive(Clone)]
pub(super) struct Facility {
    pub(super) owner: u32,
    pub(super) path: String,
    pub(super) components: Vec<(usize, &'static str)>,
}

pub(super) struct Ship {
    pub(super) owner: u32,
    pub(super) guid: Guid,
    pub(super) path: String,
    pub(super) rooms: Vec<(usize, usize, String)>,
}

pub(super) fn read(forge: &DataForge<'_>) -> Result<(HashMap<Guid, Facility>, Vec<Ship>)> {
    let mut duplicates = HashSet::new();
    let mut seen = HashSet::new();
    let mut facilities = HashMap::new();
    let mut ships = Vec::new();
    let mut builder = TreeBuilder::new();
    let probes: Vec<_> = FACILITIES
        .iter()
        .filter_map(|kind| forge.pointer_probe("Components", kind))
        .collect();
    let vehicle = forge
        .pointer_probe("Components", "VehicleComponentParams")
        .ok_or("vehicle component family")?;
    for record in forge.records() {
        if !seen.insert(record.guid()) {
            duplicates.insert(record.guid());
        }
        if record.type_name().ok() != Some("EntityClassDefinition") {
            continue;
        }
        let Ok(path) = record.record_path() else {
            continue;
        };
        let canonical = path.replace('\\', "/").to_ascii_lowercase();
        let Some(relative) = canonical.strip_prefix("libs/foundry/records/entities/") else {
            continue;
        };
        if relative.starts_with("spaceships/")
            && !p4kx_export::is_kind_leftover(p4kx_export::Kind::Ships, path, record.name()?)
            && builder.holds(&vehicle, &record)?
            && let Some(ship) = ship(&mut builder, &record, path)?
        {
            ships.push(ship);
        }
        if (relative.starts_with("scitem/") || relative.starts_with("legacyitems/"))
            && !p4kx_export::is_kind_leftover(p4kx_export::Kind::Items, path, record.name()?)
            && crate::real_archive_item_components::has_component(&mut builder, &record, &probes)?
            && let Some(facility) = facility(&mut builder, &record, path)?
        {
            facilities.insert(record.guid(), facility);
        }
    }
    facilities.retain(|guid, _| !duplicates.contains(guid));
    Ok((facilities, ships))
}

fn ship<'source>(
    builder: &mut TreeBuilder<'source>,
    record: &Record<'source>,
    path: &str,
) -> Result<Option<Ship>> {
    let tree = builder.build(record)?;
    let Some(Value::Array(components)) = tree.field("Components") else {
        return Ok(None);
    };
    let rooms = authored_rooms(components);
    Ok((!rooms.is_empty()).then(|| Ship {
        owner: record.index(),
        guid: record.guid(),
        path: path.to_owned(),
        rooms,
    }))
}

fn facility<'source>(
    builder: &mut TreeBuilder<'source>,
    record: &Record<'source>,
    path: &str,
) -> Result<Option<Facility>> {
    let tree = builder.build(record)?;
    let Some(Value::Array(components)) = tree.field("Components") else {
        return Ok(None);
    };
    let components: Vec<_> = components
        .iter()
        .enumerate()
        .filter_map(|(slot, value)| {
            let Value::Struct(node) = value else {
                return None;
            };
            FACILITIES
                .iter()
                .find(|&&kind| kind == node.type_name())
                .map(|&kind| (slot, kind))
        })
        .collect();
    Ok((!components.is_empty()).then(|| Facility {
        owner: record.index(),
        path: path.to_owned(),
        components,
    }))
}

fn authored_rooms(components: &[Value<'_>]) -> Vec<(usize, usize, String)> {
    let mut rooms = Vec::new();
    for (component, value) in components.iter().enumerate() {
        let Value::Struct(value) = value else {
            continue;
        };
        if value.type_name() != "VehicleComponentParams" {
            continue;
        }
        let Some(Value::Array(slots)) = value.field("objectContainers") else {
            continue;
        };
        for (slot, value) in slots.iter().enumerate() {
            if let Value::Struct(value) = value
                && let Some(Value::String(path)) = value.field("fileName")
                && !path.trim().is_empty()
            {
                rooms.push((component, slot, path.trim().to_owned()));
            }
        }
    }
    rooms
}
