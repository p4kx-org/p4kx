// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Loadouts in the P4KX catalog of a real Archive; runs with `--ignored` when `P4KX_DATA_P4K`
//! names one.

use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::time::{Instant, SystemTime};

use p4kx_assembly::{ArchiveFiles, Attachment, LoadoutResolver, PortNode, PortSource, PortType};
use p4kx_export::{BuiltCatalog, CatalogOptions, Kind, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const MIN_PORTS: usize = 10_000;
const ORIG_100I: &str = "6135a874-4cb1-4f49-9f29-5781e5991f2b";
const VEHICLE_ONLY: &str = "vehicle";
const PINNED_100I_TREE_PORTS: usize = 75;
const PINNED_100I_VEHICLE_ONLY: usize = 7;

fn list<'c>(catalog: &'c Value, kind: &str) -> Result<&'c [Value], Box<dyn Error>> {
    Ok(catalog[kind].as_array().ok_or("the kind is a list")?)
}

fn ports(entry: &Value) -> &[Value] {
    entry["ports"].as_array().map_or(&[], Vec::as_slice)
}

#[expect(
    clippy::print_stdout,
    reason = "the counts are the result of the verification"
)]
fn report(built: &BuiltCatalog, elapsed: std::time::Duration) {
    let counts = built.summary().loadouts();
    println!("catalog built in {elapsed:?}: {counts:?}");
    println!(
        "catalog size: {} bytes, {:?} compressed",
        built.summary().size(),
        built.summary().compressed_size()
    );
}

fn walk<'c>(ports: &'c [Value], visit: &mut impl FnMut(&'c Value)) {
    for port in ports {
        visit(port);
        walk(self::ports(port), visit);
    }
}

fn check_counts(built: &BuiltCatalog, catalog: &Value) -> TestResult {
    let counts = built.summary().loadouts();
    let loadouts = list(catalog, "loadouts")?;
    assert_eq!(
        built.summary().kind(Kind::Loadouts).map(|kind| kind.count),
        Some(loadouts.len()),
        "the kind summary"
    );
    assert_eq!(
        counts.ships + counts.items,
        loadouts.len(),
        "ships and items"
    );
    assert_eq!(
        counts.ships,
        list(catalog, "ships")?.len(),
        "every ship has a loadout"
    );
    assert!(counts.ports >= MIN_PORTS, "{} ports", counts.ports);
    let (mut ports, mut attached, mut docked, mut unresolved) = (0, 0, 0, 0);
    for entry in loadouts {
        walk(self::ports(entry), &mut |port| {
            ports += 1;
            attached += usize::from(port["unresolved"].is_null() && !port["itemGuid"].is_null());
            docked += usize::from(port["dockedVehicle"] == json!(true));
            unresolved += usize::from(!port["unresolved"].is_null());
        });
    }
    assert_eq!(
        (ports, attached, docked, unresolved),
        (
            counts.ports,
            counts.attached,
            counts.docked_vehicles,
            counts.unresolved
        ),
        "the summary counts the ports of the file"
    );
    Ok(())
}

#[expect(
    clippy::print_stdout,
    reason = "the unresolved GUIDs are the result of the verification"
)]
fn check_references(built: &BuiltCatalog, catalog: &Value) -> TestResult {
    let mut known = HashSet::new();
    for kind in [Kind::Items, Kind::Ships] {
        for entity in list(catalog, kind.name())? {
            known.insert(crate::catalog_v3_consumer::identity(entity, kind)?);
        }
    }
    let mut owners = HashSet::new();
    let mut outside = HashSet::new();
    let mut classes = HashSet::new();
    for entry in list(catalog, "loadouts")? {
        let owner = entry
            .pointer("/entityGuid/value")
            .and_then(Value::as_str)
            .ok_or("entityGuid")?;
        assert!(known.contains(owner), "owner {owner} is an item or a ship");
        assert!(owners.insert(owner), "{owner} has one loadout");
        walk(ports(entry), &mut |port| {
            if port["unresolved"].is_null()
                && let Some(guid) = port["itemGuid"].as_str()
                && !known.contains(guid)
                && outside.insert(guid)
                && let Some(class) = port["itemClassName"].as_str()
            {
                classes.insert(class);
            }
        });
    }
    assert_eq!(
        outside.len(),
        built.summary().loadouts().outside,
        "the summary counts the Entities without an entry"
    );
    let mut sample: Vec<&str> = classes.into_iter().collect();
    sample.sort_unstable();
    sample.truncate(40);
    println!(
        "{} attached Entities have no entry in items or ships, e.g. {sample:?}",
        outside.len()
    );
    Ok(())
}

fn port_name(port: &Value) -> Option<&str> {
    port.pointer("/name/value").and_then(Value::as_str)
}

fn same(tree: &[PortNode], listed: &[Value], at: &str) {
    assert!(listed.len() >= tree.len(), "{at}: ports");
    for (port, json) in tree.iter().zip(listed) {
        let here = format!("{at}/{}", port.name());
        assert_eq!(port_name(json), Some(port.name()), "{here}: name");
        let source = if port.source() == PortSource::Empty {
            VEHICLE_ONLY
        } else {
            port.source().code()
        };
        assert_eq!(json["source"], source, "{here}: source");
        assert_eq!(
            json["visibility"],
            port.visibility().code(),
            "{here}: visibility"
        );
        if let Some(definition) = port.definition() {
            assert_eq!(json["minSize"], json!(definition.min_size()), "{here}: min");
            assert_eq!(json["maxSize"], json!(definition.max_size()), "{here}: max");
            let types: Vec<&str> = json["types"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|kind| kind["type"].as_str())
                .collect();
            let wanted: Vec<&str> = definition.types().iter().map(PortType::name).collect();
            assert_eq!(types, wanted, "{here}: types");
        }
        match port.attachment() {
            Attachment::Empty => {
                assert!(json["itemGuid"].is_null(), "{here}: empty");
                same(port.nested(), ports(json), &here);
            }
            Attachment::Entity(child) => {
                assert_eq!(json["itemGuid"], child.guid().to_string(), "{here}: item");
                assert_eq!(json["itemClassName"], child.class_name(), "{here}: class");
                assert_eq!(
                    json["dockedVehicle"],
                    json!(child.is_docked_vehicle()),
                    "{here}: docked"
                );
                if child.is_expanded() {
                    assert_eq!(ports(json).len(), child.ports().len(), "{here}: below");
                    same(child.ports(), ports(json), &here);
                } else {
                    assert!(ports(json).is_empty(), "{here}: not expanded");
                }
            }
            Attachment::Unresolved(unresolved) => {
                assert_eq!(
                    json["unresolved"],
                    unresolved.error().kind(),
                    "{here}: error"
                );
            }
        }
    }
}

fn check_known_ship_row(catalog: &Value, index: u32) -> Result<&Value, Box<dyn Error>> {
    let listed = list(catalog, "loadouts")?
        .iter()
        .find(|entry| {
            entry
                .pointer("/sourceRecordIndex/value")
                .and_then(Value::as_u64)
                == Some(u64::from(index))
        })
        .ok_or("the Loadout of the selected physical ship")?;
    assert_eq!(
        crate::catalog_v3_consumer::identity(listed, Kind::Loadouts)?,
        ORIG_100I,
        "the original ship GUID"
    );
    assert_eq!(
        listed.pointer("/entityKind/value").and_then(Value::as_str),
        Some("ship"),
        "the original owner is a ship"
    );
    Ok(listed)
}

fn check_known_ship(archive: &Archive, catalog: &Value) -> TestResult {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(archive);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_files(&files)
        .stopping_at_docked_vehicles();
    let record = resolver.entities().find(ORIG_100I)?;
    let tree = resolver.resolve(&record)?;
    let listed = check_known_ship_row(catalog, record.index())?;
    same(tree.ports(), ports(listed), "100i");
    assert_eq!(
        ports(listed).len(),
        tree.ports().len(),
        "no port beyond the tree"
    );
    let vehicle_only: Vec<&PortNode> = tree
        .ports()
        .iter()
        .filter(|port| port.source() == PortSource::Empty)
        .collect();
    assert_eq!(
        vehicle_only.len(),
        PINNED_100I_VEHICLE_ONLY,
        "vehicle-only ports"
    );
    for port in &vehicle_only {
        assert!(matches!(port.attachment(), Attachment::Empty), "empty");
    }
    assert_eq!(
        tree.ports().len(),
        PINNED_100I_TREE_PORTS,
        "tree ports with the vehicle-only ones"
    );
    let top = ports(listed);
    let by_name = |name: &str| top.iter().find(|port| port_name(port) == Some(name));
    let power = by_name("hardpoint_power_plant").ok_or("the power plant port")?;
    assert_eq!(
        power["itemGuid"], "4a4a2a9c-040f-4f36-befe-7101b230fa53",
        "power plant"
    );
    assert_eq!(
        power["itemClassName"], "POWR_LPLT_S01_PowerBolt_SCItem",
        "power plant class"
    );
    let gun = by_name("hardpoint_weapon_gun_left").ok_or("the gun port")?;
    assert_eq!(gun["itemClassName"], "Mount_Gimbal_S3", "gun mount");
    assert_eq!(ports(gun).len(), 1, "the gimbal mount carries one gun port");
    let bay = by_name("hardpoint_weapon_missilebay_front").ok_or("the missile bay")?;
    assert_eq!(
        ports(bay).len(),
        2,
        "the missile rack carries two missile ports"
    );
    Ok(())
}

#[expect(
    clippy::print_stdout,
    reason = "the counts are the result of the verification"
)]
fn report_codes(catalog: &Value) -> TestResult {
    let mut unresolved: BTreeMap<&str, usize> = BTreeMap::new();
    let mut issues: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in list(catalog, "loadouts")? {
        for code in entry["issues"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            *issues.entry(code).or_default() += 1;
        }
        walk(ports(entry), &mut |port| {
            if let Some(code) = port["unresolved"].as_str() {
                *unresolved.entry(code).or_default() += 1;
            }
        });
    }
    println!("unresolved ports by code: {unresolved:?}");
    println!("loadouts with an issue, by code: {issues:?}");
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_loadouts_of_a_real_archive_resolve_and_match_the_loadout_tree_of_a_ship() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let started = Instant::now();
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    report(&built, started.elapsed());
    let catalog: Value = serde_json::from_slice(built.json())?;
    check_counts(&built, &catalog)?;
    check_references(&built, &catalog)?;
    report_codes(&catalog)?;
    check_known_ship(&archive, &catalog)
}
