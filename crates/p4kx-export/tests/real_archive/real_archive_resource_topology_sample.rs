// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::time::UNIX_EPOCH;

use p4kx_assembly::{ArchiveFiles, Attachment, EntityNode, LoadoutResolver};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Record, RecordTree, StructValue, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use super::{Declaration, TestResult};
use crate::real_archive_auto_spawns::provenance::Origins;
use crate::real_archive_item_components as components;
use crate::real_archive_item_durability as proof;
use crate::real_archive_verification::print_line;

#[path = "real_archive_topology_bindings.rs"]
mod bindings;

const PORT_CONTAINER: &str = "SItemPortContainerComponentParams";
const RESOURCE_COMPONENT: &str = "ItemResourceComponentParams";
const ITEM_RESOURCES: &str = "/configurations/resourceComponents/rows";

struct Ship<'source> {
    tree: RecordTree<'source>,
    assembly: EntityNode,
    relays: BTreeMap<usize, u32>,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn source_selected_ship_topology_matches_original_declarations_and_bindings() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let files = ArchiveFiles::new(&archive);
    let (avenger, multi_relay) = select(&forge, &files, &watch)?;
    for ship in [&avenger, &multi_relay] {
        let catalog = build(&archive, &entry, ship.tree.record_path())?;
        let origins = Origins::read(&catalog, entry.origin())?;
        let counts =
            bindings::compare(&catalog, &origins, &ship.tree, &ship.assembly, &ship.relays)?;
        print_line(&format!(
            "Resource topology: {} authored hardpoint links, {} room associations and {} source-selected relay placements",
            counts.hardpoints,
            counts.rooms,
            ship.relays.len()
        ));
    }
    let mut checked_targets = BTreeSet::new();
    for &target in multi_relay.relays.values() {
        if checked_targets.insert(target) {
            compare_relay(&archive, &entry, &forge, target)?;
        }
    }
    watch
        .check()
        .map_err(|_| "Archive changed during verification".into())
}

fn select<'source>(
    forge: &'source DataForge<'source>,
    files: &'source ArchiveFiles<'source>,
    watch: &ArchiveWatch,
) -> TestResult<(Ship<'source>, Ship<'source>)> {
    let mut resolver = LoadoutResolver::new(forge)
        .with_files(files)
        .stopping_at_docked_vehicles();
    let avenger = resolver
        .entities()
        .find("AEGS_Avenger_Titan")
        .map_err(|_| "configured Archive has no uniquely resolved acceptance ship")?;
    let mut trees = TreeBuilder::new();
    let mut relay_flags = BTreeMap::new();
    let original = trees
        .build(&avenger)
        .map_err(|_| "read original acceptance ship")?;
    let fixed = read_ship(
        &mut resolver,
        &mut trees,
        forge,
        &mut relay_flags,
        &avenger,
        original,
    )?;
    let rule = Kind::Ships.rule().ok_or("canonical Ship selection")?;
    let probe = forge
        .pointer_probe("Components", PORT_CONTAINER)
        .ok_or("original port container family")?;
    for record in forge.records() {
        watch.poll().map_err(|_| "Archive changed")?;
        let path = record
            .record_path()
            .map_err(|_| "read original Record path")?;
        if record.index() == avenger.index()
            || !rule.matches_path(path)
            || record.type_name()? != rule.type_name
            || p4kx_export::is_kind_leftover(Kind::Ships, path, record.name()?)
            || !trees
                .holds(&probe, &record)
                .map_err(|_| "inspect original ship Components")?
        {
            continue;
        }
        let tree = trees
            .build(&record)
            .map_err(|_| "read candidate ship Record")?;
        if !declared(&tree, Declaration::Hardpoint) || !declared(&tree, Declaration::Room) {
            continue;
        }
        let ship = read_ship(
            &mut resolver,
            &mut trees,
            forge,
            &mut relay_flags,
            &record,
            tree,
        )?;
        if ship.relays.len() >= 2 {
            return Ok((fixed, ship));
        }
    }
    Err("configured Archive has no independent ship with two direct, structurally identified relays".into())
}

fn declared(tree: &RecordTree<'_>, declaration: Declaration) -> bool {
    let (_, kind, member, _) = declaration.fields();
    component_slots(tree).any(|(_, source)| {
        source.type_name() == kind
            && matches!(source.field(member), Some(Original::Array(slots)) if !slots.is_empty())
    })
}

fn read_ship<'source>(
    resolver: &mut LoadoutResolver<'source>,
    trees: &mut TreeBuilder<'source>,
    forge: &'source DataForge<'source>,
    cache: &mut BTreeMap<u32, bool>,
    record: &Record<'source>,
    tree: RecordTree<'source>,
) -> TestResult<Ship<'source>> {
    let assembly = resolver
        .resolve(record)
        .map_err(|_| "resolve original ship loadout")?;
    let mut relays = BTreeMap::new();
    for (position, port) in assembly.ports().iter().enumerate() {
        let Attachment::Entity(entity) = port.attachment() else {
            continue;
        };
        if let Some(target) = entity.record_index()
            && has_relay(forge, trees, cache, target)?
        {
            relays.insert(position, target);
        }
    }
    Ok(Ship {
        tree,
        assembly,
        relays,
    })
}

fn has_relay<'source>(
    forge: &'source DataForge<'source>,
    trees: &mut TreeBuilder<'source>,
    cache: &mut BTreeMap<u32, bool>,
    index: u32,
) -> TestResult<bool> {
    if let Some(value) = cache.get(&index) {
        return Ok(*value);
    }
    let record = forge.record(index).ok_or("original attachment target")?;
    let tree = trees
        .build(&record)
        .map_err(|_| "read original resource target")?;
    let value = relay_slots(&tree).next().is_some();
    cache.insert(index, value);
    Ok(value)
}

fn component_slots<'read, 'source>(
    tree: &'read RecordTree<'source>,
) -> impl Iterator<Item = (usize, &'read StructValue<'source>)> {
    let slots: &[Original<'source>] = match tree.field("Components") {
        Some(Original::Array(slots)) => slots,
        _ => &[],
    };
    slots.iter().enumerate().filter_map(component)
}

const fn component<'read, 'source>(
    (slot, raw): (usize, &'read Original<'source>),
) -> Option<(usize, &'read StructValue<'source>)> {
    let Original::Struct(data) = raw else {
        return None;
    };
    Some((slot, data))
}

fn relay_slots<'read, 'source>(
    tree: &'read RecordTree<'source>,
) -> impl Iterator<Item = (usize, &'read Original<'source>)> {
    component_slots(tree).filter_map(|(slot, source)| {
        if source.type_name() == RESOURCE_COMPONENT
            && let Some(raw @ Original::Bool(true)) = source.field("isRelay")
        {
            return Some((slot, raw));
        }
        None
    })
}

fn compare_relay(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    index: u32,
) -> TestResult {
    let record = forge.record(index).ok_or("original relay target")?;
    let tree = record.tree().map_err(|_| "read original relay target")?;
    let catalog = build(archive, entry, tree.record_path())?;
    let rows = components::component_rows(&catalog, ITEM_RESOURCES)?;
    let origins = Origins::read(&catalog, entry.origin())?;
    let locations = crate::real_archive_auto_spawns::original::Locations::read(tree.root());
    let mut matched = 0;
    for (slot, raw) in relay_slots(&tree) {
        let row = rows
            .get(&(index, slot))
            .ok_or("selected relay resource row")?;
        let cell = components::cell(row, "isRelay")?;
        assert!(
            proof::text(row, "/ownerGuid/value")? == tree.guid().to_string()
                && cell.pointer("/value").and_then(Value::as_bool) == Some(true),
            "the contextual relay target joins its original true component discriminator"
        );
        origins.occurrence(cell, index, locations.ordinal(raw)?, "$value")?;
        matched += 1;
    }
    assert!(
        matched > 0,
        "the original relay target has structural source evidence"
    );
    print_line(&format!(
        "Resource topology: {matched} original relay component slots joined"
    ));
    Ok(())
}

fn build(archive: &Archive, entry: &DataForgeEntry, path: &str) -> TestResult<Value> {
    let filter = Filter::new(path);
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, &options)
        .map_err(|_| "build source-selected topology catalog")?;
    let catalog: Value =
        serde_json::from_slice(built.json()).map_err(|_| "read scoped catalog JSON")?;
    crate::real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    Ok(catalog)
}
