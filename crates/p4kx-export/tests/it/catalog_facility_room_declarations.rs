// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::resource_topology::RoomAssociations;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry,
    TestPort, TestStruct, TestType,
};
use serde_json::{Value, json};

use super::{TestResult, common, proof};

const DIRECT: u128 = 0x1063_2000;
const MOUNT: u128 = 0x1063_2001;
const MODULE: u128 = 0x1063_2002;
const MISSING: &str = "ObjectContainers/Ships/Fixture/room_not_in_archive.socpak";
const BROKEN: &str = "ObjectContainers/Ships/Fixture/room_invalid_utf8.socpak";
const BLANK: &str = "   ";
const PORTS: [&str; 2] = ["left", "right"];

fn database(file_field: bool) -> DataForgeBuilder {
    let ship = TestEntity::new(
        DIRECT,
        "FixtureDirectRoomHost",
        "libs/foundry/records/entities/spaceships/fixture_direct_room_host.xml",
    )
    .vehicle("", "")
    .object_container(MISSING, "", [0.0; 3], [0.0; 3])
    .object_container(BLANK, "", [0.0; 3], [0.0; 3])
    .object_container(BROKEN, "", [0.0; 3], [0.0; 3])
    .record();
    let [left, right] = PORTS;
    let mount = TestEntity::new(
        MOUNT,
        "FixtureMountedRoomHost",
        "libs/foundry/records/entities/spaceships/fixture_mounted_room_host.xml",
    )
    .vehicle("", "")
    .port(TestPort::new(left))
    .port(TestPort::new(right))
    .loadout(TestLoadout::Manual(
        PORTS
            .into_iter()
            .map(|port| TestLoadoutEntry::reference(port, MODULE))
            .collect(),
    ))
    .record();
    let module = TestEntity::new(
        MODULE,
        "FixtureRoomModule",
        "libs/foundry/records/entities/scitem/fixture/room_module.xml",
    )
    .attach_type("Misc")
    .item_container(BLANK)
    .record();
    let database = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8))
        .record(ship)
        .record(mount)
        .record(module);
    if file_field {
        database
    } else {
        database.structure(
            TestStruct::new("SVehicleObjectContainerParams").field("boneName", TestType::String),
        )
    }
}

fn fixture_archive(file_field: bool) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture_room_declarations.p4k");
    let mut bytes = database(file_field).build();
    if file_field {
        let offset = bytes
            .windows(BROKEN.len())
            .position(|value| value == BROKEN.as_bytes())
            .ok_or("original room filename bytes")?;
        *bytes.get_mut(offset).ok_or("original room filename byte")? = 0xff;
    }
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("Data/Game2.dcb", bytes).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

fn catalog(file_field: bool) -> TestResult<Value> {
    let (_folder, archive) = fixture_archive(file_field)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        build_catalog(&archive, options.with_full_provenance())?.json(),
    )?)
}

fn direct<'row>(coverage: &'row Value, slot: usize, state: &str) -> TestResult<&'row Value> {
    let room = proof::at(coverage, "/room")?;
    proof::present(
        proof::at(room, "/owner")?,
        "ownerRecordIndex",
        &json!(0),
        Some(true),
    )?;
    proof::present(room, "objectContainerOrdinal", &json!(slot), Some(true))?;
    assert!(
        room["loadoutPosition"].is_null(),
        "a direct ship room is independent of a Loadout path"
    );
    assert!(
        proof::rows(coverage, "/containers")?.is_empty(),
        "no Archive container was admitted"
    );
    proof::cell(coverage, "fileName", state)
}

fn attached(coverage: &Value, port: u32) -> TestResult {
    let room = proof::at(coverage, "/room")?;
    proof::present(
        proof::at(room, "/owner")?,
        "ownerRecordIndex",
        &json!(2),
        Some(true),
    )?;
    proof::present(
        room,
        "loadoutPosition",
        &json!({"loadoutRecordIndex": 1, "portOrdinals": [port]}),
        Some(true),
    )?;
    proof::present(coverage, "fileName", &json!(BLANK), Some(true))?;
    assert!(
        proof::rows(coverage, "/containers")?.is_empty(),
        "a blank filename opens no container"
    );
    assert!(
        proof::rows(coverage, "/findings")?.is_empty(),
        "no path lookup was attempted"
    );
    Ok(())
}

#[test]
fn coverage_retains_original_room_filenames_and_ordered_physical_slots() -> TestResult {
    let catalog = catalog(true)?;
    let [named, blank, broken, left, right] =
        proof::rows(&catalog, "/placements/facilityInteriorCoverage")?
    else {
        return Err("three direct declarations and two attached occurrences".into());
    };
    assert_eq!(
        direct(named, 0, "present")?["value"],
        MISSING,
        "the authored target remains present"
    );
    let [finding] = proof::rows(named, "/findings")? else {
        return Err("one missing container report".into());
    };
    assert_eq!(
        finding["kind"], "missingContainer",
        "only a nonblank valid path reached the Archive lookup"
    );
    assert_eq!(
        direct(blank, 1, "present")?["value"],
        BLANK,
        "the authored whitespace survives"
    );
    let unreadable = direct(broken, 2, "unreadable")?;
    assert!(
        !proof::uses(&catalog, unreadable)?.is_empty(),
        "the unreadable value retains its original site"
    );
    for row in [blank, broken] {
        assert!(
            proof::rows(row, "/findings")?.is_empty(),
            "an unusable declaration never creates an Archive lookup failure"
        );
    }
    attached(left, 0)?;
    attached(right, 1)?;
    proof::closure(&catalog)
}

#[test]
fn coverage_retains_slots_with_an_absent_filename_field() -> TestResult {
    let catalog = catalog(false)?;
    let [one, two, three, left, right] =
        proof::rows(&catalog, "/placements/facilityInteriorCoverage")?
    else {
        return Err("three absent-name direct declarations and two attached occurrences".into());
    };
    for (slot, row) in [one, two, three].into_iter().enumerate() {
        let absent = direct(row, slot, "absent")?;
        assert!(
            !proof::uses(&catalog, absent)?.is_empty(),
            "the original room slot establishes its missing field"
        );
        assert!(
            proof::rows(row, "/findings")?.is_empty(),
            "an absent locator did not trigger an Archive lookup"
        );
    }
    attached(left, 0)?;
    attached(right, 1)?;
    proof::closure(&catalog)
}

#[test]
fn physical_room_readers_keep_direct_and_mounted_declarations_in_their_own_scope() -> TestResult {
    let (_folder, archive) = fixture_archive(false)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let vehicle = CatalogRecordSelection::RecordIndex(0);
    let item = CatalogRecordSelection::RecordIndex(2);

    parsed.with_room_associations(vehicle, None, |snapshot| {
        verify_direct_room_associations(snapshot.rows())
    })??;
    parsed.with_item_room_associations(item, None, |snapshot| -> TestResult {
        let [room] = snapshot.rows() else {
            return Err("one physical mounted room owner".into());
        };
        assert_eq!(
            room.owner().record_index().value(),
            Some(&2),
            "an attached item owns its declaration independently of its host"
        );
        assert_eq!(
            room.object_container().value().copied(),
            Some(BLANK),
            "the selected item retains its original scalar filename"
        );
        Ok(())
    })??;
    parsed.with_item_room_associations(vehicle, None, |snapshot| {
        assert!(
            snapshot.rows().is_empty(),
            "an item scope cannot substitute an unrelated physical owner"
        );
    })?;
    Ok(())
}

fn verify_direct_room_associations(rows: &[RoomAssociations<'_>]) -> TestResult {
    let [room] = rows else {
        return Err("one physical direct room owner".into());
    };
    assert_eq!(
        room.owner().record_index().value(),
        Some(&0),
        "the selected physical ship owns the direct declarations"
    );
    let slots = room
        .object_containers()
        .value()
        .ok_or("original object container slots")?;
    assert_eq!(
        slots.len(),
        3,
        "the direct declarations exist without a resolved attachment path"
    );
    for (ordinal, slot) in slots.iter().enumerate() {
        let room = slot.value().ok_or("admitted original room slot")?;
        assert_eq!(
            room.container_ordinal().value(),
            Some(&u32::try_from(ordinal)?),
            "the original direct slot order survives"
        );
        assert!(
            room.file_name().value().is_none(),
            "a missing filename does not erase its owning slot"
        );
    }
    Ok(())
}

#[test]
fn coverage_query_keeps_direct_and_mounted_rooms_on_their_physical_roots() -> TestResult {
    use p4kx_export::catalog_queries::equipment::read_facility_interior_coverage;
    use p4kx_extraction::SourceValueState;

    let (_folder, archive) = fixture_archive(false)?;
    let direct = read_facility_interior_coverage(&archive, 0, None)?;
    assert_eq!(
        direct.root_state,
        SourceValueState::Present,
        "the ship with direct rooms is independently admitted"
    );
    assert_eq!(
        direct.rows.len(),
        3,
        "all physical direct declarations survive missing names"
    );
    assert!(
        direct
            .rows
            .iter()
            .all(|row| row.owner().record_index().value() == Some(&0)
                && row.loadout_position().is_none()),
        "the direct room owner is its own physical Ship"
    );

    let mounted = read_facility_interior_coverage(&archive, 1, None)?;
    let [left, right] = mounted.rows.as_slice() else {
        return Err("two exact attached occurrences".into());
    };
    for (row, port) in [(left, 0), (right, 1)] {
        assert_eq!(
            row.owner().record_index().value(),
            Some(&2),
            "both ports use the original item room owner"
        );
        let position = row
            .loadout_position()
            .and_then(|cell| cell.value())
            .ok_or("physical mounting path")?;
        assert_eq!(
            position.loadout_record_index(),
            1,
            "the actual second Ship hosts the attached room"
        );
        assert_eq!(
            position.port_ordinals(),
            [port],
            "authored mounting order stays visible"
        );
    }
    let foreign = read_facility_interior_coverage(&archive, 2, None)?;
    assert_eq!(
        foreign.root_state,
        SourceValueState::Absent,
        "an Item with a room does not become a Ship root"
    );
    assert!(
        foreign.rows.is_empty(),
        "the query never borrows rooms from another physical root"
    );
    Ok(())
}
