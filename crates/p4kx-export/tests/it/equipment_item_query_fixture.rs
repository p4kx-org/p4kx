// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const OWNER: u128 = 0x9650;
pub(super) const MAKER: u128 = 0x9651;
pub(super) const ROWS: &str = "/definitions/items/rows";
const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const ATTACHMENT: &str = "SAttachableComponentParams";
const DEFINITION: &str = "SItemDefinition";
const LOCALIZATION: &str = "SCItemLocalization";
const PHYSICS: &str = "SEntityPhysicsControllerParams";
const RIGID: &str = "SEntityRigidPhysicsControllerParams";
const PURCHASE: &str = "SCItemPurchasableParams";
const MANUFACTURER: &str = "SCItemManufacturer";
pub(super) const MASS_NAN: u32 = 0x7fc0_0127;
pub(super) const GRADE_NAN: u64 = 0x7ff8_0000_0000_0137;

#[derive(Clone, Copy, Debug)]
pub(super) enum Layout {
    Complete,
    AuthoredManufacturer,
    MissingFields,
    NullAttachment,
    UnreadableName,
    Nonfinite,
    NonWeapon,
}

pub(super) const ALL: [Layout; 7] = [
    Layout::Complete,
    Layout::AuthoredManufacturer,
    Layout::MissingFields,
    Layout::NullAttachment,
    Layout::UnreadableName,
    Layout::Nonfinite,
    Layout::NonWeapon,
];

pub(super) fn database(layout: Layout) -> DataForgeBuilder {
    let structs = [
        TestStruct::new(ENTITY).array("Components", pointer(COMPONENT)),
        TestStruct::new(COMPONENT),
        TestStruct::new(ATTACHMENT)
            .parent(COMPONENT)
            .field("AttachDef", pointer(DEFINITION)),
        definition(layout),
        TestStruct::new(LOCALIZATION)
            .field("Name", TestType::Locale)
            .field("Description", TestType::Locale),
        TestStruct::new(PHYSICS)
            .parent(COMPONENT)
            .field("PhysType", class(RIGID)),
        TestStruct::new(RIGID).field("Mass", TestType::Single),
        purchasable(layout),
        TestStruct::new(MANUFACTURER),
    ];
    structs
        .into_iter()
        .fold(
            DataForgeBuilder::new().version(8),
            DataForgeBuilder::structure,
        )
        .record(item("alpha", "@shared", layout))
        .record(item("beta", "@later_name", layout))
        .record(TestRecord::new(
            MAKER,
            MANUFACTURER,
            "SCItemManufacturer.Supplier",
            "libs/foundry/records/scitemmanufacturer/fixture/supplier.xml",
        ))
}

fn definition(layout: Layout) -> TestStruct {
    let definition = TestStruct::new(DEFINITION)
        .field("Type", TestType::String)
        .field("Size", TestType::Int32);
    if matches!(layout, Layout::MissingFields) {
        return definition;
    }
    let definition = definition
        .field("Tags", TestType::String)
        .field("Grade", TestType::Double)
        .field("Localization", class(LOCALIZATION));
    let target = TestType::ReferenceTo(MANUFACTURER.to_owned());
    if matches!(layout, Layout::AuthoredManufacturer) {
        definition.array("Manufacturer", target)
    } else {
        definition.field("Manufacturer", target)
    }
}

fn purchasable(layout: Layout) -> TestStruct {
    let source = TestStruct::new(PURCHASE)
        .parent(COMPONENT)
        .field("displayType", TestType::Locale);
    if matches!(layout, Layout::MissingFields) {
        source
    } else {
        source.field("displayName", TestType::Locale)
    }
}

fn item(name: &str, key: &str, layout: Layout) -> TestRecord {
    let mass = if matches!(layout, Layout::Nonfinite) {
        f32::from_bits(MASS_NAN)
    } else {
        -0.0
    };
    TestRecord::new(
        OWNER,
        ENTITY,
        format!("{ENTITY}.{name}"),
        format!("libs/foundry/records/entities/scitem/fixture/equipment_{name}.xml"),
    )
    .value(
        "Components",
        TestValue::Array(vec![
            TestValue::Null,
            attachment(layout, key, 1.25),
            node(TestInstance::new(PHYSICS).value(
                "PhysType",
                node(TestInstance::new(RIGID).value("Mass", TestValue::Single(mass))),
            )),
            attachment(layout, key, 2.5),
            purchase(layout, key),
        ]),
    )
}

fn attachment(layout: Layout, key: &str, grade: f64) -> TestValue {
    let original_type = if matches!(layout, Layout::NonWeapon) {
        "PowerPlant"
    } else {
        "WeaponPersonal"
    };
    let mut source = TestInstance::new(DEFINITION)
        .value("Type", TestValue::text(original_type))
        .value("Size", TestValue::Int32(2));
    if !matches!(layout, Layout::MissingFields) {
        source = source
            .value("Tags", TestValue::text("TagOne TagTwo"))
            .value(
                "Grade",
                TestValue::Double(if matches!(layout, Layout::Nonfinite) {
                    f64::from_bits(GRADE_NAN)
                } else {
                    grade
                }),
            )
            .value("Localization", localization(layout, key))
            .value("Manufacturer", manufacturer(layout));
    }
    let definition = if matches!(layout, Layout::NullAttachment) {
        TestValue::Null
    } else {
        node(source)
    };
    node(TestInstance::new(ATTACHMENT).value("AttachDef", definition))
}

fn localization(layout: Layout, key: &str) -> TestValue {
    let name = if matches!(layout, Layout::UnreadableName) {
        TestValue::Raw((u32::MAX - 1).to_le_bytes().to_vec())
    } else {
        TestValue::text(key)
    };
    node(
        TestInstance::new(LOCALIZATION)
            .value("Name", name)
            .value("Description", TestValue::text(key)),
    )
}

fn manufacturer(layout: Layout) -> TestValue {
    if matches!(layout, Layout::AuthoredManufacturer) {
        TestValue::Array(vec![
            TestValue::Reference(MAKER),
            TestValue::Null,
            TestValue::Reference(MAKER),
        ])
    } else {
        TestValue::Reference(MAKER)
    }
}

fn purchase(layout: Layout, key: &str) -> TestValue {
    let source = TestInstance::new(PURCHASE).value("displayType", TestValue::text(key));
    node(if matches!(layout, Layout::MissingFields) {
        source
    } else {
        source.value("displayName", TestValue::text(key))
    })
}

const fn node(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}
fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}
