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
    item_types(builder, weapon_type())
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

fn item_types(builder: DataForgeBuilder, weapon: TestStruct) -> DataForgeBuilder {
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
        .structure(weapon)
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

fn weapon_type() -> TestStruct {
    component("SCItemWeaponComponentParams")
        .field("ammoContainerRecord", reference("EntityClassDefinition"))
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

#[derive(Clone, Copy)]
pub(super) enum MagazineCase {
    Resolved,
    BadLinks,
}

pub(super) const PRIMARY_MAGAZINE: u128 = MAGAZINE;
pub(super) const SECONDARY_MAGAZINE: u128 = 0x8125;
pub(super) const MISSING_MAGAZINE: u128 = 0x8126;

pub(super) fn magazine_database(case: MagazineCase) -> DataForgeBuilder {
    magazine_records(magazine_builder(), secondary_magazines(case))
}

pub(super) fn heat_magazine_database(raw_value: f32) -> DataForgeBuilder {
    let builder = magazine_builder()
        .enumeration("EComparisonMode", ["Equal"])
        .structure(
            TestStruct::new("SFloatNumberComparer")
                .field("mode", TestType::Enum("EComparisonMode".to_owned()))
                .field("value", TestType::Single),
        )
        .structure(
            TestStruct::new("SWeaponConditionHeatLevel")
                .parent("SWeaponConditionBase")
                .field(
                    "comparer",
                    TestType::Class("SFloatNumberComparer".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("SWeaponConditionAll")
                .parent("SWeaponConditionBase")
                .array("conditions", pointer("SWeaponConditionBase")),
        );
    let heat = node(
        TestInstance::new("SWeaponConditionHeatLevel").value(
            "comparer",
            node(
                TestInstance::new("SFloatNumberComparer")
                    .value("mode", TestValue::text("Equal"))
                    .value("value", TestValue::Single(raw_value)),
            ),
        ),
    );
    let all = TestInstance::new("SWeaponConditionAll").value(
        "conditions",
        TestValue::Array(vec![
            heat,
            node(TestInstance::new("SWeaponConditionIsHeldEntity")),
        ]),
    );
    magazine_records(
        builder,
        vec![secondary(Some(SECONDARY_MAGAZINE), Some(all))],
    )
}

fn magazine_builder() -> DataForgeBuilder {
    let builder = ammo_types(DataForgeBuilder::new().version(8));
    let weapon = weapon_type()
        .array(
            "secondaryAmmoContainers",
            TestType::Class("SSecondaryAmmoContainerParams".to_owned()),
        )
        .field("ShouldIgnorePrimaryAmmoContainer", TestType::Bool);
    item_types(builder, weapon)
        .structure(
            TestStruct::new("SSecondaryAmmoContainerParams")
                .field("condition", pointer("SWeaponConditionBase"))
                .field("ammoContainerRecord", reference("EntityClassDefinition")),
        )
        .structure(TestStruct::new("SWeaponConditionBase"))
        .structure(
            TestStruct::new("SWeaponConditionTargetingFire")
                .parent("SWeaponConditionBase")
                .field("comparer", TestType::Bool),
        )
        .structure(TestStruct::new("SWeaponConditionIsHeldEntity").parent("SWeaponConditionBase"))
}

fn magazine_records(builder: DataForgeBuilder, secondaries: Vec<TestValue>) -> DataForgeBuilder {
    builder
        .record(ammo(BULLET, "fixture_bullet").value("speed", TestValue::Single(12.5)))
        .record(ammo(TACHYON, "fixture_tachyon").value("speed", TestValue::Single(26.0)))
        .record(item(
            DIRECT,
            "fixture_direct",
            vec![
                weapon_with_magazines(MAGAZINE, true, Vec::new()),
                container(200),
            ],
        ))
        .record(item(
            PERSONAL,
            "fixture_personal",
            vec![weapon_with_magazines(MAGAZINE, false, secondaries)],
        ))
        .record(item(MAGAZINE, "fixture_magazine", vec![container(30)]))
        .record(item(
            SECONDARY_MAGAZINE,
            "fixture_secondary_magazine",
            vec![container(27)],
        ))
}

fn weapon_with_magazines(
    primary: u128,
    ignore_primary: bool,
    secondaries: Vec<TestValue>,
) -> TestValue {
    node(
        TestInstance::new("SCItemWeaponComponentParams")
            .value("ammoContainerRecord", TestValue::Reference(primary))
            .value(
                "ShouldIgnorePrimaryAmmoContainer",
                TestValue::Bool(ignore_primary),
            )
            .value("secondaryAmmoContainers", TestValue::Array(secondaries)),
    )
}

fn secondary_magazines(case: MagazineCase) -> Vec<TestValue> {
    let (first, second, third) = match case {
        MagazineCase::Resolved => (
            Some(SECONDARY_MAGAZINE),
            Some(SECONDARY_MAGAZINE),
            Some(MAGAZINE),
        ),
        MagazineCase::BadLinks => (Some(MISSING_MAGAZINE), Some(TACHYON), None),
    };
    vec![
        secondary(
            first,
            Some(
                TestInstance::new("SWeaponConditionTargetingFire")
                    .value("comparer", TestValue::Bool(false)),
            ),
        ),
        secondary(
            second,
            Some(TestInstance::new("SWeaponConditionIsHeldEntity")),
        ),
        secondary(third, None),
    ]
}

fn secondary(magazine: Option<u128>, condition: Option<TestInstance>) -> TestValue {
    let mut source = TestInstance::new("SSecondaryAmmoContainerParams");
    if let Some(guid) = magazine {
        source = source.value("ammoContainerRecord", TestValue::Reference(guid));
    }
    if let Some(condition) = condition {
        source = source.value("condition", node(condition));
    }
    node(source)
}
