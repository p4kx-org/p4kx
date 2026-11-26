// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const SHARED_TEXT: &str = "fixture_item_shared";
pub(super) const DISPLAY_NAME: &str = "fixture_item_display";
pub(super) const DISPLAY_TYPE: &str = "fixture_item_type";
const OWNER: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const ATTACHMENT: &str = "SAttachableComponentParams";
const ATTACH_DEF: &str = "SItemDefinition";
const LOCALE: &str = "SCItemLocalization";
const PHYSICS: &str = "SEntityPhysicsControllerParams";
const RIGID: &str = "SEntityRigidPhysicsControllerParams";
const PURCHASE: &str = "SCItemPurchasableParams";
const MAKER: &str = "SCItemManufacturer";
const HIGH: u128 = 0x1177_9005;
const LOW: u128 = 0x1177_9002;
const MAKER_GUID: u128 = 0x1177_9006;

pub(super) fn database() -> DataForgeBuilder {
    schema(TestType::ReferenceTo(MAKER.to_owned()))
        .record(item(
            HIGH,
            "FixtureFirst",
            vec![
                TestValue::Null,
                attachment(2, TestValue::Reference(MAKER_GUID)),
                physics(),
                attachment(-3, TestValue::Reference(0)),
                purchase(),
            ],
        ))
        .record(item(LOW, "FixtureLowerGuid", vec![purchase()]))
        .record(item(
            HIGH,
            "FixtureAlias",
            vec![attachment(5, TestValue::Reference(MAKER_GUID))],
        ))
        .record(manufacturer("FixturePrimary", "PRIM"))
        .record(manufacturer("FixtureAlias", "ALIAS"))
}

pub(super) fn authored_manufacturer() -> DataForgeBuilder {
    schema(TestType::String).record(item(
        HIGH,
        "FixtureManufacturerText",
        vec![attachment(2, TestValue::text("PRIM"))],
    ))
}

fn schema(manufacturer_type: TestType) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(OWNER)
                .array("Components", TestType::StrongPointer(COMPONENT.to_owned())),
        )
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new(ATTACHMENT)
                .parent(COMPONENT)
                .field("AttachDef", class(ATTACH_DEF)),
        )
        .structure(
            TestStruct::new(ATTACH_DEF)
                .field("Type", TestType::String)
                .field("SubType", TestType::String)
                .field("Size", TestType::UInt16)
                .field("Grade", TestType::Int32)
                .field("Tags", TestType::String)
                .field("RequiredTags", TestType::String)
                .field("Manufacturer", manufacturer_type)
                .field("Localization", class(LOCALE)),
        )
        .structure(
            TestStruct::new(LOCALE)
                .field("Name", TestType::Locale)
                .field("Description", TestType::Locale),
        )
        .structure(
            TestStruct::new(PHYSICS)
                .parent(COMPONENT)
                .field("PhysType", class(RIGID)),
        )
        .structure(TestStruct::new(RIGID).field("Mass", TestType::Single))
        .structure(
            TestStruct::new(PURCHASE)
                .parent(COMPONENT)
                .field("displayName", TestType::Locale)
                .field("displayType", TestType::Locale),
        )
        .structure(TestStruct::new(MAKER).field("Code", TestType::String))
}

fn item(guid: u128, name: &str, components: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        guid,
        OWNER,
        format!("{OWNER}.{name}"),
        format!("libs/foundry/records/entities/scitem/fixture/{name}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn manufacturer(name: &str, code: &str) -> TestRecord {
    TestRecord::new(
        MAKER_GUID,
        MAKER,
        format!("{MAKER}.{name}"),
        format!("libs/foundry/records/scitemmanufacturer/fixture/{name}.xml"),
    )
    .value("Code", TestValue::text(code))
}

fn attachment(grade: i32, manufacturer: TestValue) -> TestValue {
    let localization = TestInstance::new(LOCALE)
        .value("Name", locale(SHARED_TEXT))
        .value("Description", locale(SHARED_TEXT));
    let definition = TestInstance::new(ATTACH_DEF)
        .value("Type", TestValue::text("WeaponPersonal"))
        .value("SubType", TestValue::text("Rifle"))
        .value("Size", TestValue::UInt16(3))
        .value("Grade", TestValue::Int32(grade))
        .value("Tags", TestValue::text("fixture fit tags"))
        .value("RequiredTags", TestValue::text("fixture required tags"))
        .value("Manufacturer", manufacturer)
        .value("Localization", TestValue::Struct(localization));
    TestValue::Struct(
        TestInstance::new(ATTACHMENT).value("AttachDef", TestValue::Struct(definition)),
    )
}

fn physics() -> TestValue {
    TestValue::Struct(TestInstance::new(PHYSICS).value(
        "PhysType",
        TestValue::Struct(TestInstance::new(RIGID).value("Mass", TestValue::Single(-0.0))),
    ))
}

fn purchase() -> TestValue {
    TestValue::Struct(
        TestInstance::new(PURCHASE)
            .value("displayName", locale(DISPLAY_NAME))
            .value("displayType", locale(DISPLAY_TYPE)),
    )
}

fn class(target: &str) -> TestType {
    TestType::Class(target.to_owned())
}

fn locale(key: &str) -> TestValue {
    TestValue::text(format!("@{key}"))
}
