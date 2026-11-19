// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{DOCKED_XML, LOADOUT_XML, PATCH_XML, SHIP_XML, database_for, ship_with_same_vehicle};
use crate::commodity_v3_fixture::TestResult;

const SHIP_XML_BYTES: &[u8] = br#"<Vehicle name="FixtureTopology"><Parts><Part name="mount_a" class="ItemPort"><ItemPort flags="invisible"/></Part><Part name="vehicle_only" class="ItemPort"><ItemPort/></Part></Parts></Vehicle>"#;
const DOCKED_XML_BYTES: &[u8] = br#"<Vehicle name="FixtureDocked"><Parts><Part name="docked_port" class="ItemPort"><ItemPort/></Part></Parts></Vehicle>"#;
const LOADOUT_XML_BYTES: &[u8] = br#"<Loadout xmlns:fixture="urn:fixture"><Items><Item fixture:portName="mount_a" portName="unused_alias" itemName="SyntheticRelay"/><Item portName="mount_b" itemName="SyntheticRelay"/><Item portName="deck" itemName="RELAY_2slot"/><Item portName="loose" itemName="RELAY_2slot"/><Item portName="hangar" itemName="FixtureDocked"/></Items></Loadout>"#;
const PATCH_XML_BYTES: &[u8] = br#"<Modifications><Part id="vehicle-id" name="vehicle_only"><ItemPort/></Part><PortDonor id="fixture-port" requiredTags="$Patch Cargo" maxSize="7" display_name=""/></Modifications>"#;
const SHIP_PATCH_BASE: &[u8] = br#"<Vehicle name="FixtureTopology"><Parts><Part id="vehicle-id" name="old_vehicle_port" class="ItemPort"><ItemPort id="fixture-port" requiredTags="Base" maxSize="2" display_name="@base"/></Part></Parts><Modifications><Modification name="variant" patchFile="fixture_topology_patch.xml"/></Modifications></Vehicle>"#;
const SHIP_ELEM_BASE: &str = r#"<Vehicle name="FixtureTopology"><Parts><Part id="vehicle-id" name="old_vehicle_port" class="ItemPort"><ItemPort id="fixture-port" portTags="Base" minSize="1" display_name="@base"/></Part></Parts><Modifications><Modification name="variant"><Elems><Elem idRef="vehicle-id" name="name" value="vehicle_only"/><Elem idRef="fixture-port" name="portTags" value="$Elem Cargo"/><Elem idRef="fixture-port" name="minSize" value="4"/><Elem idRef="fixture-port" name="display_name" value="@inline_port"/></Elems></Modification></Modifications></Vehicle>"#;

#[derive(Clone, Copy)]
pub(in super::super) enum GraphSource {
    Database,
    Vehicle,
    Loadout,
    Patch,
    Elem,
    ElemWithoutDisplayValue,
}

pub(in super::super) fn graph_archive() -> TestResult<(tempfile::TempDir, Archive)> {
    graph_archive_for(GraphSource::Database)
}

pub(in super::super) fn graph_archive_for(
    source: GraphSource,
) -> TestResult<(tempfile::TempDir, Archive)> {
    graph_archive_from(source, None)
}

pub(in super::super) fn graph_archive_with_vehicle(
    contents: &[u8],
    encrypted: bool,
) -> TestResult<(tempfile::TempDir, Archive)> {
    graph_archive_from(GraphSource::Elem, Some((contents, encrypted)))
}

fn graph_archive_from(
    source: GraphSource,
    override_vehicle: Option<(&[u8], bool)>,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("topology.p4k");
    let mut records = database_for(source);
    if override_vehicle.is_some() {
        records = records.record(ship_with_same_vehicle());
    }
    let database = TestEntry::zstd("Data\\Game2.dcb", records.build());
    let database = if matches!(source, GraphSource::Database) {
        database.encrypted()
    } else {
        database
    };
    let without_display_value;
    let vehicle = match source {
        GraphSource::Patch => SHIP_PATCH_BASE,
        GraphSource::Elem => SHIP_ELEM_BASE.as_bytes(),
        GraphSource::ElemWithoutDisplayValue => {
            without_display_value = SHIP_ELEM_BASE.replace(r#" value="@inline_port""#, "");
            without_display_value.as_bytes()
        }
        _ => SHIP_XML_BYTES,
    };
    let (vehicle, encrypted) = override_vehicle.unwrap_or((
        vehicle,
        matches!(
            source,
            GraphSource::Vehicle | GraphSource::Elem | GraphSource::ElemWithoutDisplayValue
        ),
    ));
    let mut archive = ArchiveBuilder::new()
        .entry(database)
        .entry(xml_entry(SHIP_XML, vehicle, encrypted))
        .entry(xml_entry(DOCKED_XML, DOCKED_XML_BYTES, false));
    match source {
        GraphSource::Loadout => {
            archive = archive.entry(xml_entry(LOADOUT_XML, LOADOUT_XML_BYTES, true));
        }
        GraphSource::Patch => archive = archive.entry(xml_entry(PATCH_XML, PATCH_XML_BYTES, true)),
        _ => {}
    }
    archive.write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((directory, archive))
}

fn xml_entry(relative: &str, contents: &[u8], encrypted: bool) -> TestEntry {
    let path = format!("Data\\{}", relative.replace('/', "\\"));
    let entry = TestEntry::stored(path, contents.to_vec());
    if encrypted { entry.encrypted() } else { entry }
}
