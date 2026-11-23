// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::read_facility_interior_coverage;
use p4kx_extraction::{Archive, SourceValueState};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestContainer, TestEntity, TestEntry, TestObjectContainer,
};

use super::super::FACILITY_INTERIOR_COVERAGE;
use super::{Route, TestResult};

const LOCATOR: &str = r"ObjectContainers\Ships\Fixture\room.socpak";
const ENTRY: &str = r"Data\ObjectContainers\Ships\Fixture\room.socpak";

fn fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("facility-interior.p4k");
    let database = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8))
        .record(
            TestEntity::new(
                1,
                "FixtureRoomHost",
                "libs/foundry/records/entities/spaceships/fixture_room_host.xml",
            )
            .vehicle("", "")
            .object_container(LOCATOR, "", [0.0; 3], [0.0; 3])
            .record(),
        )
        .build();
    let room = TestContainer::new()
        .entry(TestEntry::stored(
            "room.xml",
            TestObjectContainer::new("room").cryxml(),
        ))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database).encrypted())
        .entry(TestEntry::stored(ENTRY, room))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((directory, archive))
}

pub(super) fn read() -> TestResult<Route> {
    let (_directory, archive) = fixture()?;
    let sdk = read_facility_interior_coverage(&archive, 0, None)?;
    assert_eq!(
        sdk.root_state,
        SourceValueState::Present,
        "the scope admits the physical Ship"
    );
    let [row] = sdk.rows.as_slice() else {
        return Err("one original direct room declaration".into());
    };
    assert_eq!(
        row.owner().record_index().value(),
        Some(&0),
        "the direct room belongs to the selected physical owner"
    );
    assert_eq!(
        row.file_name().value().map(String::as_str),
        Some(LOCATOR),
        "the locator keeps the original filename"
    );
    assert_eq!(
        row.file_name().origin().contains_encrypted_source(),
        Some(true),
        "the original encrypted DataForge input supplies the filename"
    );
    assert!(
        row.loadout_position().is_none(),
        "the room comes directly from the selected Ship"
    );
    let physical = archive
        .entries()
        .iter()
        .find(|entry| entry.path().eq_ignore_ascii_case(ENTRY))
        .ok_or("the actual synthetic room entry")?;
    let wire = serde_json::to_value(row)?;
    assert_eq!(
        wire.pointer("/containers/0/container/value/entryIndex")
            .and_then(serde_json::Value::as_u64),
        Some(u64::try_from(physical.index())?),
        "the scoped walker opened the exact physical room entry"
    );
    Route::queried_rows(
        FACILITY_INTERIOR_COVERAGE,
        &sdk.rows,
        "installed cold memory, producer parity, source-use closure and scope enumeration are unverified",
    )
}
