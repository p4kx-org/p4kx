// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestChf, TestEntity, TestEntry, TestInstance, TestItemPort,
    TestPort, TestStruct, TestType, TestValue, entity_schema,
};

use super::{TestResult, lowered_hash, motion_option_assertions, subadb_graph_assertions};

pub(super) const BODY: u128 = 0x0101_0101_0101_0101_0101_0101_0101_0101;
pub(super) const WEAPON_ONE: u128 = 0x0202_0202_0202_0202_0202_0202_0202_0202;
pub(super) const WEAPON_TWO: u128 = 0x0303_0303_0303_0303_0303_0303_0303_0303;

pub(crate) fn fixture_with_entries(
    extra: impl IntoIterator<Item = TestEntry>,
) -> TestResult<(tempfile::TempDir, Archive)> {
    fixture_with(subadb_graph_assertions::ROOT_LINKS, extra)
}

pub(super) fn fixture_with(
    root_links: &str,
    extra: impl IntoIterator<Item = TestEntry>,
) -> TestResult<(tempfile::TempDir, Archive)> {
    fixture_with_motion(motion_option_assertions::entries(root_links), extra)
}

pub(super) fn fixture_with_motion(
    motion: [TestEntry; 3],
    extra: impl IntoIterator<Item = TestEntry>,
) -> TestResult<(tempfile::TempDir, Archive)> {
    fixture_with_forge(motion, extra, forge(|actor| actor))
}

pub(super) fn fixture_with_forge(
    motion: [TestEntry; 3],
    extra: impl IntoIterator<Item = TestEntry>,
    forge: Vec<u8>,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("player.p4k");
    let mut male = TestChf::v7();
    male.ports = Some(port(
        1,
        1,
        vec![
            port(lowered_hash("nested"), 0, vec![]),
            port(
                lowered_hash("stowed"),
                2,
                vec![port(lowered_hash("nested"), 0, vec![])],
            ),
        ],
    ));
    let mut female = TestChf::v8();
    female.ports = Some(port(1, 1, vec![port(lowered_hash("missing"), 4, vec![])]));
    let loadout = br#"<Loadout><Items><Item portName="root" itemName=""><Items>
        <Item portName="stowed" itemName="SharedWeapon"/>
        <Item portName="missing" itemName="MissingClass"/>
        <Item portName="unspecified"><Items/></Item>
    </Items></Item><Item portName="second" itemName="SyntheticBody"><Items>
        <Item portName="nested" itemName="SharedWeapon"/>
        <Item portName="unknown" itemName=""/>
    </Items></Item></Items><Inventory><InventoryItem itemName="Other"/></Inventory></Loadout>"#;
    let archive = ArchiveBuilder::new()
        .entry(
            TestEntry::stored(
                "Data/Libs/CharacterCustomizer/masculinedefault.chf",
                male.to_bytes(),
            )
            .encrypted(),
        )
        .entry(TestEntry::stored(
            "Data/Libs/CharacterCustomizer/femininedefault.chf",
            female.to_bytes(),
        ))
        .entry(TestEntry::stored("Data/Game2.dcb", forge).encrypted())
        .entry(TestEntry::stored(
            "Data/Scripts/Loadouts/Player/DefaultLoadouts/sample.xml",
            loadout,
        ));
    motion
        .into_iter()
        .chain(subadb_graph_assertions::entries())
        .chain(extra)
        .fold(archive, ArchiveBuilder::entry)
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

pub(super) fn forge(tune: impl FnOnce(TestEntity) -> TestEntity) -> Vec<u8> {
    let animated = TestInstance::new("SAnimatedCharacterComponentParams")
        .value(
            "actionController",
            TestValue::text("Animations/test/controller.xml"),
        )
        .value(
            "animDatabase3P",
            TestValue::text("Animations/test/player.adb"),
        );
    let actor = TestEntity::new(4, "Player", "libs/foundry/records/entities/test/player")
        .component(TestValue::Struct(animated));
    entity_schema(DataForgeBuilder::new())
        .structure(
            TestStruct::new("SAnimatedCharacterComponentParams")
                .parent("DataForgeComponentParams")
                .field("actionController", TestType::String)
                .field("animDatabase3P", TestType::String),
        )
        .record(tune(actor).record())
        .record(
            TestEntity::new(
                BODY,
                "SyntheticBody",
                "libs/foundry/records/entities/test/body",
            )
            .port(
                TestPort::new("nested")
                    .skin()
                    .helper("BodyAnchor", [1.0, 2.0, 3.0], [0.0; 3]),
            )
            .port(
                TestPort::new("stowed")
                    .flags("invisible")
                    .helper("StowOne", [0.0; 3], [0.0; 3]),
            )
            .port(TestPort::new("STOWED").helper("StowTwo", [0.0; 3], [0.0; 3]))
            .record(),
        )
        .record(
            TestEntity::new(
                WEAPON_ONE,
                "SharedWeapon",
                "libs/foundry/records/entities/test/weapon-a",
            )
            .port(TestPort::new("nested").helper("WeaponAnchor", [0.0; 3], [0.0; 3]))
            .record(),
        )
        .record(
            TestEntity::new(
                WEAPON_TWO,
                "SharedWeapon",
                "libs/foundry/records/entities/test/weapon-b",
            )
            .record(),
        )
        .build()
}

const fn port(port: u32, item: u8, children: Vec<TestItemPort>) -> TestItemPort {
    TestItemPort {
        port,
        item: [item; 16],
        tail_count: 0,
        children,
    }
}
