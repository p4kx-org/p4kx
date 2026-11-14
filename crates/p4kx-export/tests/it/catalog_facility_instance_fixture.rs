// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestLoadout,
    TestLoadoutEntry, TestPort, TestRecord, TestStruct, TestType, TestValue,
};

use crate::commodity_v3_fixture::TestResult;

pub(super) const SHIP: u128 = 0x1063_0200;
pub(super) const SEAT: u128 = 0x1063_0201;
pub(super) const DECK: u128 = 0x1063_0202;
pub(super) const DOCKED: u128 = 0x1063_0203;
const CONTAINER: u128 = 0x1063_0204;
const MISSING: u128 = 0x1063_0205;

const SEAT_COMPONENT: &str = "SCItemSeatParams";
const STORAGE_COMPONENT: &str = "SCItemInventoryContainerComponentParams";
const CONTAINER_DEFINITION: &str = "InventoryContainer";
const LOADOUT_XML: &str = "Scripts/Entities/Vehicles/Loadouts/fixture_facility.xml";
const LOADOUT_BYTES: &[u8] =
    br#"<Loadout><Items><Item portName="deck" itemName="FixtureFacilityDeck"/></Items></Loadout>"#;
const MOUNTS: [(&str, u128); 5] = [
    ("seat_left", SEAT),
    ("seat_right", SEAT),
    ("deck", DECK),
    ("hangar", DOCKED),
    ("missing", MISSING),
];

pub(super) fn database() -> DataForgeBuilder {
    with_loadout(loadout(MOUNTS))
}

pub(super) fn encrypted_xml_archive() -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("fixture_facility_xml.p4k");
    let database = with_loadout(TestLoadout::Xml(LOADOUT_XML.to_owned()));
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", database.build()))
        .entry(TestEntry::stored(format!("Data/{LOADOUT_XML}"), LOADOUT_BYTES).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((directory, archive))
}

fn with_loadout(ship_loadout: TestLoadout) -> DataForgeBuilder {
    p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8))
        .enumeration("SeatTypes", ["FixtureSeat"])
        .structure(
            TestStruct::new(SEAT_COMPONENT)
                .parent("DataForgeComponentParams")
                .field("seatType", TestType::Enum("SeatTypes".to_owned())),
        )
        .structure(
            TestStruct::new(STORAGE_COMPONENT)
                .parent("DataForgeComponentParams")
                .field(
                    "containerParams",
                    TestType::ReferenceTo(CONTAINER_DEFINITION.to_owned()),
                ),
        )
        .structure(TestStruct::new(CONTAINER_DEFINITION))
        .record(ship(ship_loadout))
        .record(
            TestEntity::new(
                SEAT,
                "FixtureSeat",
                "libs/foundry/records/entities/scitem/fixture/seat.xml",
            )
            .attach_type("Misc")
            .component(seat())
            .record(),
        )
        .record(deck())
        .record(
            TestEntity::new(
                DOCKED,
                "FixtureDocked",
                "libs/foundry/records/entities/spaceships/fixture_docked.xml",
            )
            .vehicle("", "")
            .attach_type("NOITEM_Vehicle")
            .port(TestPort::new("docked_seat"))
            .loadout(loadout([("docked_seat", SEAT)]))
            .component(seat())
            .record(),
        )
        .record(TestRecord::new(
            CONTAINER,
            CONTAINER_DEFINITION,
            "InventoryContainer.FixtureDeck",
            "libs/foundry/records/inventory/fixture_deck.xml",
        ))
}

fn ship(ship_loadout: TestLoadout) -> TestRecord {
    let mut ship = TestEntity::new(
        SHIP,
        "FixtureFacilityHost",
        "libs/foundry/records/entities/spaceships/fixture_facility_host.xml",
    )
    .vehicle("", "");
    for (port, _) in MOUNTS {
        ship = ship.port(TestPort::new(port));
    }
    ship.loadout(ship_loadout).record()
}

fn deck() -> TestRecord {
    TestEntity::new(
        DECK,
        "FixtureFacilityDeck",
        "libs/foundry/records/entities/scitem/fixture/deck.xml",
    )
    .attach_type("Misc")
    .port(TestPort::new("inner_seat"))
    .loadout(loadout([("inner_seat", SEAT)]))
    .component(TestValue::Struct(
        TestInstance::new(STORAGE_COMPONENT)
            .value("containerParams", TestValue::Reference(CONTAINER)),
    ))
    .record()
}

fn seat() -> TestValue {
    TestValue::Struct(
        TestInstance::new(SEAT_COMPONENT).value("seatType", TestValue::text("FixtureSeat")),
    )
}

fn loadout<const N: usize>(mounts: [(&str, u128); N]) -> TestLoadout {
    TestLoadout::Manual(
        mounts
            .into_iter()
            .map(|(port, target)| TestLoadoutEntry::reference(port, target))
            .collect(),
    )
}
