// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const OWNER: u128 = 0x1086_2000;
pub(super) const FIRST: u128 = 0x1086_2001;
pub(super) const SECOND: u128 = 0x1086_2002;
pub(super) const MISSING: u128 = 0x1086_20ff;
const COMPONENT_BASE: &str = "DataForgeComponentParams";
const COMPONENT: &str = "EntityComponentMissionBoardUIProvider";
const UNRELATED: &str = "FixtureBoardUnrelatedComponent";
const BOARD_BASE: &str = "MissionBoardType_Base";
const CAREER: &str = "MissionBoardType_Career";
const NOTICE: &str = "MissionBoardType_Notice";
const FUTURE: &str = "FixtureFutureMissionBoardType";
const REPUTATION: &str = "FactionReputation";

pub(super) fn database() -> DataForgeBuilder {
    database_at("libs/foundry/records/fixture_board/owner.xml")
}

pub(super) fn shared_item_database() -> DataForgeBuilder {
    database_at("libs/foundry/records/entities/scitem/fixture_board/owner.xml")
}

fn database_at(path: &str) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(REPUTATION))
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer(COMPONENT_BASE.to_owned()),
        ))
        .structure(TestStruct::new(COMPONENT_BASE))
        .structure(TestStruct::new(UNRELATED).parent(COMPONENT_BASE))
        .structure(
            TestStruct::new(COMPONENT)
                .parent(COMPONENT_BASE)
                .field("boardType", TestType::StrongPointer(BOARD_BASE.to_owned())),
        )
        .structure(TestStruct::new(BOARD_BASE))
        .structure(
            TestStruct::new(CAREER)
                .parent(BOARD_BASE)
                .field(
                    "guildFactionReputation",
                    TestType::ReferenceTo(REPUTATION.to_owned()),
                )
                .array("factions", TestType::ReferenceTo(REPUTATION.to_owned())),
        )
        .structure(TestStruct::new(NOTICE).parent(BOARD_BASE))
        .structure(
            TestStruct::new(FUTURE)
                .parent(BOARD_BASE)
                .field("futureScore", TestType::Single),
        )
        .record(faction(FIRST, "first"))
        .record(faction(SECOND, "second"))
        .record(owner(path))
}

fn faction(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        REPUTATION,
        format!("FactionReputation.Fixture_{name}"),
        format!("libs/foundry/records/factions/factionreputation/board_fixture_{name}.xml"),
    )
}

fn owner(path: &str) -> TestRecord {
    let career = TestInstance::new(CAREER)
        .value("guildFactionReputation", TestValue::Reference(SECOND))
        .value(
            "factions",
            TestValue::Array(vec![
                TestValue::Reference(FIRST),
                TestValue::Null,
                TestValue::Reference(MISSING),
                TestValue::Reference(FIRST),
            ]),
        );
    let future = TestInstance::new(FUTURE).value(
        "futureScore",
        TestValue::Single(f32::from_bits(0x7fc0_1086)),
    );
    let components = vec![
        TestValue::Struct(TestInstance::new(UNRELATED)),
        provider(TestValue::Struct(career)),
        provider(TestValue::Struct(TestInstance::new(NOTICE))),
        TestValue::Null,
        provider(TestValue::Struct(future)),
        provider(TestValue::Null),
    ];
    TestRecord::new(
        OWNER,
        "EntityClassDefinition",
        "EntityClassDefinition.FixtureBoardOwner",
        path,
    )
    .value("Components", TestValue::Array(components))
}

fn provider(board_type: TestValue) -> TestValue {
    TestValue::Struct(TestInstance::new(COMPONENT).value("boardType", board_type))
}
