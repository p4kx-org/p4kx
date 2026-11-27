// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Loadout census and direct Assembly parity for an exact retained Catalog and its installed Archive.

use std::error::Error;

use p4kx_assembly::{ArchiveFiles, Attachment, LoadoutResolver, PortNode, PortSource, PortType};
use p4kx_export::Kind;
use p4kx_extraction::{Archive, DataForgeEntry};
use serde_json::{Value, json};

use crate::real_archive_catalog_table_parity::{
    BoundMeasurement, ProducerSeal, with_bound_measurement_from_producer,
};

type TestResult = Result<(), Box<dyn Error>>;

const ORIG_100I: &str = "6135a874-4cb1-4f49-9f29-5781e5991f2b";
const VEHICLE_ONLY: &str = "vehicle";
const PINNED_100I_TREE_PORTS: usize = 75;
const PINNED_100I_VEHICLE_ONLY: usize = 7;

fn ports(entry: &Value) -> &[Value] {
    entry["ports"].as_array().map_or(&[], Vec::as_slice)
}

fn port_name(port: &Value) -> Option<&str> {
    port.pointer("/name/value").and_then(Value::as_str)
}

fn port_field<'json>(
    data: &'json Value,
    field: &str,
    at: &str,
) -> Result<&'json Value, Box<dyn Error>> {
    data.pointer(field)
        .ok_or_else(|| format!("{at}: missing Loadout port field {field}").into())
}

fn same(tree: &[PortNode], listed: &[Value], at: &str) -> TestResult {
    assert!(listed.len() >= tree.len(), "{at}: ports");
    for (port, json) in tree.iter().zip(listed) {
        let here = format!("{at}/{}", port.name());
        assert_eq!(port_name(json), Some(port.name()), "{here}: name");
        check_port_shape(port, json, &here)?;
        check_port_attachment(port, json, &here)?;
    }
    Ok(())
}

fn check_port_shape(port: &PortNode, json: &Value, at: &str) -> TestResult {
    let source = if port.source() == PortSource::Empty {
        VEHICLE_ONLY
    } else {
        port.source().code()
    };
    assert_eq!(
        port_field(json, "/source", at)?.as_str(),
        Some(source),
        "{at}: source"
    );
    assert_eq!(
        port_field(json, "/visibility/value", at)?.as_str(),
        Some(port.visibility().code()),
        "{at}: visibility"
    );
    if let Some(definition) = port.definition() {
        for (field, size) in [
            ("/minSize/value", definition.min_size()),
            ("/maxSize/value", definition.max_size()),
        ] {
            assert_eq!(port_field(json, field, at)?, &json!(size), "{at}: {field}");
        }
        check_port_types(json, definition.types(), at)?;
    }
    Ok(())
}

fn check_port_types(json: &Value, expected: &[PortType], at: &str) -> TestResult {
    let expected: Vec<_> = expected
        .iter()
        .map(PortType::name)
        .filter(|name| !name.is_empty())
        .collect();
    let types = match port_field(json, "/types/value", at)? {
        Value::Array(types) => types.as_slice(),
        Value::Null if expected.is_empty() => &[],
        _ => return Err(format!("{at}: expected /types/value array").into()),
    };
    let mut names = Vec::new();
    for kind in types {
        match port_field(kind, "/type/value", at)? {
            Value::String(name) => names.push(name.as_str()),
            Value::Null => {}
            _ => return Err(format!("{at}: expected string or null at /type/value").into()),
        }
    }
    assert_eq!(names, expected, "{at}: types");
    Ok(())
}

fn check_port_attachment(port: &PortNode, json: &Value, at: &str) -> TestResult {
    match port.attachment() {
        Attachment::Empty => {
            assert!(
                port_field(json, "/itemGuid/value", at)?.is_null(),
                "{at}: empty"
            );
            same(port.nested(), ports(json), at)?;
        }
        Attachment::Entity(child) => {
            assert_eq!(
                port_field(json, "/itemGuid/value", at)?,
                &json!(child.guid().to_string()),
                "{at}: item"
            );
            assert_eq!(
                port_field(json, "/itemClassName/value", at)?.as_str(),
                Some(child.class_name()),
                "{at}: class"
            );
            assert_eq!(
                port_field(json, "/dockedVehicle", at)?.as_bool(),
                Some(child.is_docked_vehicle()),
                "{at}: docked"
            );
            if child.is_expanded() {
                assert_eq!(ports(json).len(), child.ports().len(), "{at}: below");
                same(child.ports(), ports(json), at)?;
            } else {
                assert!(ports(json).is_empty(), "{at}: not expanded");
            }
        }
        Attachment::Unresolved(unresolved) => {
            assert_eq!(
                port_field(json, "/unresolved", at)?.as_str(),
                Some(unresolved.error().kind()),
                "{at}: error"
            );
        }
    }
    Ok(())
}

fn check_known_ship_row(listed: &Value, tree: &[PortNode]) -> TestResult {
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
    same(tree, ports(listed), "100i")?;
    assert_eq!(ports(listed).len(), tree.len(), "no port beyond the tree");
    let vehicle_only: Vec<&PortNode> = tree
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
        tree.len(),
        PINNED_100I_TREE_PORTS,
        "tree ports with the vehicle-only ones"
    );
    check_pinned_attachments(ports(listed))
}

fn check_pinned_attachments(top: &[Value]) -> TestResult {
    let by_name = |name: &str| top.iter().find(|port| port_name(port) == Some(name));
    let power = by_name("hardpoint_power_plant").ok_or("the power plant port")?;
    assert_eq!(
        port_field(power, "/itemGuid/value", "hardpoint_power_plant")?.as_str(),
        Some("4a4a2a9c-040f-4f36-befe-7101b230fa53"),
        "power plant"
    );
    assert_eq!(
        port_field(power, "/itemClassName/value", "hardpoint_power_plant")?.as_str(),
        Some("POWR_LPLT_S01_PowerBolt_SCItem"),
        "power plant class"
    );
    let gun = by_name("hardpoint_weapon_gun_left").ok_or("the gun port")?;
    assert_eq!(
        port_field(gun, "/itemClassName/value", "hardpoint_weapon_gun_left")?.as_str(),
        Some("Mount_Gimbal_S3"),
        "gun mount"
    );
    assert_eq!(ports(gun).len(), 1, "the gimbal mount carries one gun port");
    let bay = by_name("hardpoint_weapon_missilebay_front").ok_or("the missile bay")?;
    assert_eq!(
        ports(bay).len(),
        2,
        "the missile rack carries two missile ports"
    );
    Ok(())
}

fn check_known_ship(
    archive: &Archive,
    measurement: &BoundMeasurement,
    seal: &ProducerSeal,
) -> TestResult {
    let (index, tree) = {
        let entry = DataForgeEntry::read(archive)?;
        let forge = entry.open()?;
        let files = ArchiveFiles::new(archive);
        let mut resolver = LoadoutResolver::new(&forge)
            .with_files(&files)
            .stopping_at_docked_vehicles();
        let record = resolver.entities().find(ORIG_100I)?;
        (record.index(), resolver.resolve(&record)?)
    };
    measurement.verify_loadout_census(index, seal, |row| check_known_ship_row(row, tree.ports()))
}

#[test]
#[ignore = "same Build: configure the installed Archive, complete Full Catalog and replay receipt"]
fn the_loadouts_of_a_real_archive_resolve_and_match_the_loadout_tree_of_a_ship() -> TestResult {
    let seal = ProducerSeal::read()?;
    with_bound_measurement_from_producer(seal.producer_revision(), |archive, measurement| {
        check_known_ship(archive, measurement, &seal)
    })
}
