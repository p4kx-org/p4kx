// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const BULLET: u128 = 0x8120;
pub(super) const TACHYON: u128 = 0x8121;
pub(super) const DIRECT: u128 = 0x8122;
pub(super) const PERSONAL: u128 = 0x8123;
const MAGAZINE: u128 = 0x8124;

pub(super) fn database(impact: f32) -> DataForgeBuilder {
    let builder = ammo_types(DataForgeBuilder::new().version(8));
    item_types(builder)
        .record(
            ammo(BULLET, "fixture_bullet")
                .value("speed", TestValue::Single(12.5))
                .value("lifetime", TestValue::Single(0.0))
                .value("size", TestValue::UInt8(3))
                .value(
                    "conversionRate",
                    node(
                        TestInstance::new("SCentiCargoUnit")
                            .value("centiSCU", TestValue::Int32(-7)),
                    ),
                )
                .value(
                    "projectileParams",
                    node(
                        TestInstance::new("BulletProjectileParams")
                            .value("impactRadius", TestValue::Single(impact)),
                    ),
                ),
        )
        .record(
            ammo(TACHYON, "fixture_tachyon").value(
                "projectileParams",
                node(
                    TestInstance::new("TachyonProjectileParams")
                        .value("zeroDamageRange", TestValue::Single(2.0)),
                ),
            ),
        )
        .record(item(
            DIRECT,
            "fixture_direct",
            vec![weapon(None), container(200)],
        ))
        .record(item(
            PERSONAL,
            "fixture_personal",
            vec![weapon(Some(MAGAZINE))],
        ))
        .record(item(MAGAZINE, "fixture_magazine", vec![container(30)]))
}

fn ammo_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new("AmmoParams")
                .field("speed", TestType::Single)
                .field("lifetime", TestType::Single)
                .field("size", TestType::UInt8)
                .field("conversionRate", pointer("SBaseCargoUnit"))
                .field("projectileParams", pointer("ProjectileParams")),
        )
        .structure(TestStruct::new("SBaseCargoUnit"))
        .structure(
            TestStruct::new("SCentiCargoUnit")
                .parent("SBaseCargoUnit")
                .field("centiSCU", TestType::Int32),
        )
        .structure(TestStruct::new("ProjectileParams"))
        .structure(
            TestStruct::new("BulletProjectileParams")
                .parent("ProjectileParams")
                .field("impactRadius", TestType::Single),
        )
        .structure(
            TestStruct::new("TachyonProjectileParams")
                .parent("ProjectileParams")
                .field("zeroDamageRange", TestType::Single),
        )
}

fn item_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .enumeration("FixtureItemType", ["WeaponGun"])
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("Components", pointer("DataForgeComponentParams")),
        )
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            component("SAttachableComponentParams")
                .field("AttachDef", TestType::Class("SItemDefinition".to_owned())),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", TestType::Enum("FixtureItemType".to_owned())),
        )
        .structure(
            component("SCItemWeaponComponentParams")
                .field("ammoContainerRecord", reference("EntityClassDefinition")),
        )
        .structure(
            component("SAmmoContainerComponentParams")
                .field("ammoParamsRecord", reference("AmmoParams"))
                .field("maxAmmoCount", TestType::Int32),
        )
}

fn ammo(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "AmmoParams",
        format!("AmmoParams.{name}"),
        format!("libs/foundry/records/ammoparams/fixture/{name}.xml"),
    )
}

fn item(guid: u128, name: &str, extra: Vec<TestValue>) -> TestRecord {
    let definition =
        node(TestInstance::new("SItemDefinition").value("Type", TestValue::text("WeaponGun")));
    let mut components = vec![node(
        TestInstance::new("SAttachableComponentParams").value("AttachDef", definition),
    )];
    components.extend(extra);
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{name}"),
        format!("libs/foundry/records/entities/scitem/ships/{name}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn weapon(magazine: Option<u128>) -> TestValue {
    let source = TestInstance::new("SCItemWeaponComponentParams");
    node(match magazine {
        Some(guid) => source.value("ammoContainerRecord", TestValue::Reference(guid)),
        None => source,
    })
}

fn container(capacity: i32) -> TestValue {
    node(
        TestInstance::new("SAmmoContainerComponentParams")
            .value("ammoParamsRecord", TestValue::Reference(BULLET))
            .value("maxAmmoCount", TestValue::Int32(capacity)),
    )
}

fn component(name: &str) -> TestStruct {
    TestStruct::new(name).parent("DataForgeComponentParams")
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn reference(name: &str) -> TestType {
    TestType::ReferenceTo(name.to_owned())
}

const fn node(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}
