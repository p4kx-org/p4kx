// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestLoadout, TestLoadoutEntry, TestPort,
    TestStruct, TestType, TestValue,
};

pub(super) const OWNER: u128 = 0x1063_0401;
pub(super) const OUTFIT: &str = "SEntityComponentOutfitHangerParams";
pub(super) const LOGOUT: &str = "LogoutComponentParams";
pub(super) const ANIMATION: &str = "FixtureOutfitAnimation";
const POINT: &str = "SInteractionPointParams";
const INTERACTION: &str = "SSharedInteractionParams";
const LOGOUT_ROW: &str = "LogoutInteractionParams";
const OUTFIT_TYPE: &str = "TypeSubtypeParams";
const ANIMATION_BASE: &str = "SAnimatedOutfitSwapData";
const COMPONENT: &str = "DataForgeComponentParams";

pub(super) fn database() -> DataForgeBuilder {
    let database = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8));
    let owner = TestEntity::new(
        OWNER,
        "FixtureCapabilityOwner",
        "libs/foundry/records/entities/scitem/fixture/capability_owner.xml",
    )
    .attach_type("Misc")
    .component(TestValue::Struct(outfit()))
    .component(TestValue::Struct(logout()))
    .component(TestValue::Struct(TestInstance::new(LOGOUT)))
    .port(
        TestPort::new("fixture_holder")
            .sizes(1, 3)
            .tags("FixtureStorage", "FixturePersonal")
            .item_type("WeaponPersonal", ["FixtureSubtypeA", "FixtureSubtypeB"]),
    );
    let mut ship = TestEntity::new(
        0x1063_0402,
        "FixtureCapabilityShip",
        "libs/foundry/records/entities/spaceships/fixture_capability_ship.xml",
    )
    .vehicle("", "");
    let mut attachments = Vec::new();
    for port in ["fixture_left", "fixture_right"] {
        ship = ship.port(TestPort::new(port));
        attachments.push(TestLoadoutEntry::reference(port, OWNER));
    }
    let ship = ship.loadout(TestLoadout::Manual(attachments));
    schema(database)
        .record(owner.record())
        .record(ship.record())
}

fn schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    let builder = builder
        .enumeration("EItemType", ["FixtureTypeA", "FixtureTypeB"])
        .enumeration("EItemSubType", ["FixtureSubtypeA", "FixtureSubtypeB"])
        .structure(TestStruct::new(INTERACTION).field("fixtureFlag", TestType::Bool))
        .structure(TestStruct::new(POINT).field("fixtureFlag", TestType::Bool))
        .structure(TestStruct::new(ANIMATION_BASE))
        .structure(
            TestStruct::new(ANIMATION)
                .parent(ANIMATION_BASE)
                .field("fixtureDuration", TestType::Single),
        )
        .structure(
            TestStruct::new(OUTFIT_TYPE)
                .field("itemType", TestType::Enum("EItemType".to_owned()))
                .field("itemSubType", TestType::Enum("EItemSubType".to_owned())),
        )
        .structure(outfit_schema());
    builder
        .structure(
            TestStruct::new(LOGOUT_ROW)
                .field("logoutInteraction", weak(INTERACTION))
                .field("logoutInteractionPoint", weak(POINT))
                .field("respawnInteraction", weak(INTERACTION))
                .field("respawnInteractionPoint", weak(POINT)),
        )
        .structure(
            TestStruct::new(LOGOUT)
                .parent(COMPONENT)
                .array("logoutInteractions", strong(LOGOUT_ROW)),
        )
}

fn outfit_schema() -> TestStruct {
    let shape = TestStruct::new(OUTFIT)
        .parent(COMPONENT)
        .field("fixtureInteraction", strong(INTERACTION))
        .field("fixturePoint", strong(POINT));
    let shape = [
        "equipInteraction",
        "hangInteraction",
        "swapInteraction",
        "swapAllInteraction",
        "equipAllInteraction",
        "hangAllInteraction",
        "disguiseSwapAllInteraction",
    ]
    .into_iter()
    .fold(shape, |shape, name| shape.field(name, weak(INTERACTION)));
    shape
        .array(
            "itemPortTypeSubtypes",
            TestType::Class(OUTFIT_TYPE.to_owned()),
        )
        .field("animatedOutfitSwap", strong(ANIMATION_BASE))
        .field("animatedOutfitHang", strong(ANIMATION_BASE))
        .field("destroyPlayerItems", TestType::Bool)
}

fn outfit() -> TestInstance {
    let restriction = |kind, subtype| {
        TestValue::Struct(
            TestInstance::new(OUTFIT_TYPE)
                .value("itemType", TestValue::text(kind))
                .value("itemSubType", TestValue::text(subtype)),
        )
    };
    TestInstance::new(OUTFIT)
        .value(
            "fixtureInteraction",
            TestValue::Struct(TestInstance::new(INTERACTION).named("fixture-action")),
        )
        .value(
            "fixturePoint",
            TestValue::Struct(TestInstance::new(POINT).named("fixture-point")),
        )
        .value("equipInteraction", TestValue::pointer("fixture-action"))
        .value(
            "disguiseSwapAllInteraction",
            TestValue::pointer("fixture-action"),
        )
        .value(
            "itemPortTypeSubtypes",
            TestValue::Array(vec![
                restriction("FixtureTypeB", "FixtureSubtypeA"),
                restriction("FixtureTypeA", "FixtureSubtypeB"),
            ]),
        )
        .value(
            "animatedOutfitSwap",
            TestValue::Struct(
                TestInstance::new(ANIMATION).value("fixtureDuration", TestValue::Single(-0.0)),
            ),
        )
        .value("destroyPlayerItems", TestValue::Bool(false))
}

fn logout() -> TestInstance {
    let first = TestInstance::new(LOGOUT_ROW)
        .value("logoutInteraction", TestValue::pointer("fixture-action"))
        .value(
            "logoutInteractionPoint",
            TestValue::pointer("fixture-point"),
        );
    let second = TestInstance::new(LOGOUT_ROW)
        .value("respawnInteraction", TestValue::pointer("fixture-action"))
        .value(
            "respawnInteractionPoint",
            TestValue::pointer("fixture-point"),
        );
    TestInstance::new(LOGOUT).value(
        "logoutInteractions",
        TestValue::Array(vec![
            TestValue::Struct(first),
            TestValue::Null,
            TestValue::Struct(second),
        ]),
    )
}

fn strong(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

fn weak(target: &str) -> TestType {
    TestType::WeakPointer(target.to_owned())
}
