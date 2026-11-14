// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestLoadout,
    TestLoadoutEntry, TestPort, TestRecord, TestStruct, TestType, TestValue,
};

use crate::commodity_v3_fixture::TestResult;

pub(super) const KNOWN: u128 = 0x1062_0001;
pub(super) const SUIT: u128 = 0x1062_0002;
pub(super) const BOTH: u128 = 0x1062_0003;
pub(super) const FUTURE: u128 = 0x1062_0004;
pub(super) const PARAMETERS: u128 = 0x1062_0005;
pub(super) const SHIP: u128 = 0x1062_0006;
pub(super) const MISSING: u128 = 0x1062_0007;
pub(super) const SIGNAL_BITS: u32 = 0x7fc0_1062;
pub(super) const DAMAGE_WORDS: [(&str, &str, f32); 6] = [
    ("DamagePhysical", "physical", 1.0),
    ("DamageEnergy", "energy", 2.0),
    ("DamageDistortion", "distortion", 3.0),
    ("DamageThermal", "thermal", 4.0),
    ("DamageBiochemical", "biochemical", 5.0),
    ("DamageStun", "stun", 6.0),
];

const ARMOR: &str = "SCItemVehicleArmorParams";
const SUIT_ARMOR: &str = "SCItemSuitArmorParams";
const BASE: &str = "DamageBase";
const INFO: &str = "DamageInfo";
const PARAMS: &str = "DamageParams";
const FUTURE_DAMAGE: &str = "FixtureFutureDamage";
const MACRO: &str = "DamageMacro";
const PENETRATION: &str = "ArmorPenetrationResistanceParams";
const DEFLECTION: &str = "ArmorDeflectionParams";
const CURVE: &str = "BezierCurve";
const POINT: &str = "Vec2";

const XML: &str = r#"<Vehicle name="FixtureArmored">
    <Parts>
        <Part name="armor_mount" class="ItemPort"><ItemPort flags="invisible"/></Part>
        <Part name="lost_mount" class="ItemPort"><ItemPort/></Part>
    </Parts>
</Vehicle>"#;

#[derive(Clone, Copy)]
pub(super) enum Layout {
    Complete,
    BrokenCurve,
}

pub(super) fn database(layout: Layout) -> DataForgeBuilder {
    let builder = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8));
    armor_schema(damage_schema(builder), layout)
        .record(
            item(KNOWN, "FixtureVehicleArmor")
                .component(armor(layout, node(damage_info(0.0))))
                .record(),
        )
        .record(
            item(SUIT, "FixtureSuit")
                .component(node(TestInstance::new(SUIT_ARMOR)))
                .record(),
        )
        .record(
            item(BOTH, "FixtureDualArmor")
                .component(node(TestInstance::new(SUIT_ARMOR)))
                .component(armor(layout, TestValue::Null))
                .record(),
        )
        .record(
            item(FUTURE, "FixtureFutureArmor")
                .component(armor(
                    layout,
                    node(
                        TestInstance::new(FUTURE_DAMAGE)
                            .value("fixtureCoefficient", TestValue::Single(f32::NEG_INFINITY)),
                    ),
                ))
                .record(),
        )
        .record(
            item(PARAMETERS, "FixtureMacroArmor")
                .component(armor(
                    layout,
                    node(
                        TestInstance::new(PARAMS)
                            .value("damageMacro", TestValue::Reference(MISSING))
                            .value("damageTotal", TestValue::Single(0.0)),
                    ),
                ))
                .record(),
        )
}

pub(super) fn loadout_archive() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture_armor.p4k");
    let database = database(Layout::Complete).record(ship());
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database.build()).encrypted())
        .entry(TestEntry::stored(
            "Data\\Scripts\\Entities\\Vehicles\\Implementations\\Xml\\fixture_armored.xml",
            XML.as_bytes().to_vec(),
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

fn ship() -> TestRecord {
    TestEntity::new(
        SHIP,
        "FixtureArmored",
        "libs/foundry/records/entities/spaceships/fixture_armored.xml",
    )
    .vehicle(
        "Scripts/Entities/Vehicles/Implementations/Xml/fixture_armored.xml",
        "",
    )
    .port(TestPort::new("armor_mount").default_item(FUTURE))
    .port(TestPort::new("lost_mount"))
    .loadout(TestLoadout::Manual(vec![
        TestLoadoutEntry::reference("armor_mount", KNOWN),
        TestLoadoutEntry::reference("lost_mount", MISSING),
    ]))
    .record()
}

fn item(guid: u128, name: &str) -> TestEntity {
    TestEntity::new(
        guid,
        name,
        format!("libs/foundry/records/entities/scitem/ships/armor/{name}.xml"),
    )
    .attach_type("Misc")
}

fn damage_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    let info = DAMAGE_WORDS.into_iter().fold(
        TestStruct::new(INFO).parent(BASE),
        |structure, (source, _, _)| structure.field(source, TestType::Single),
    );
    builder
        .structure(TestStruct::new(BASE))
        .structure(info)
        .structure(TestStruct::new(MACRO))
        .structure(
            TestStruct::new(PARAMS)
                .parent(BASE)
                .field("damageMacro", TestType::ReferenceTo(MACRO.to_owned()))
                .field("damageTotal", TestType::Single),
        )
        .structure(
            TestStruct::new(FUTURE_DAMAGE)
                .parent(BASE)
                .field("fixtureCoefficient", TestType::Single),
        )
}

fn armor_schema(builder: DataForgeBuilder, layout: Layout) -> DataForgeBuilder {
    let curve_storage = match layout {
        Layout::Complete => class(CURVE),
        Layout::BrokenCurve => TestType::ReferenceTo(CURVE.to_owned()),
    };
    builder
        .structure(
            TestStruct::new(ARMOR)
                .parent("DataForgeComponentParams")
                .field("signalInfrared", TestType::Single)
                .field("signalElectromagnetic", TestType::Single)
                .field("signalCrossSection", TestType::Single)
                .field("damageMultiplier", TestType::StrongPointer(BASE.to_owned()))
                .field("armorPenetrationResistance", class(PENETRATION))
                .field("armorDeflection", class(DEFLECTION)),
        )
        .structure(TestStruct::new(SUIT_ARMOR).parent("DataForgeComponentParams"))
        .structure(
            TestStruct::new(PENETRATION)
                .field("basePenetrationReduction", TestType::Single)
                .field("penetrationAbsorptionForType", class(INFO)),
        )
        .structure(
            TestStruct::new(DEFLECTION)
                .field("deflectionValue", class(INFO))
                .field("healthCurve", curve_storage),
        )
        .structure(
            TestStruct::new(CURVE)
                .array("points", class(POINT))
                .field("useLUT", TestType::Bool),
        )
        .structure(
            TestStruct::new(POINT)
                .field("x", TestType::Single)
                .field("y", TestType::Single),
        )
}

fn armor(layout: Layout, multiplier: TestValue) -> TestValue {
    let penetration = TestInstance::new(PENETRATION)
        .value("basePenetrationReduction", TestValue::Single(0.0))
        .value("penetrationAbsorptionForType", node(damage_info(10.0)));
    let deflection = TestInstance::new(DEFLECTION)
        .value("deflectionValue", node(damage_info(20.0)))
        .value("healthCurve", curve(layout));
    node(
        TestInstance::new(ARMOR)
            .value(
                "signalInfrared",
                TestValue::Single(f32::from_bits(SIGNAL_BITS)),
            )
            .value("signalElectromagnetic", TestValue::Single(0.0))
            .value("signalCrossSection", TestValue::Single(1.0))
            .value("damageMultiplier", multiplier)
            .value("armorPenetrationResistance", node(penetration))
            .value("armorDeflection", node(deflection)),
    )
}

fn damage_info(offset: f32) -> TestInstance {
    DAMAGE_WORDS
        .into_iter()
        .fold(TestInstance::new(INFO), |instance, (source, _, value)| {
            instance.value(source, TestValue::Single(value + offset))
        })
}

fn curve(layout: Layout) -> TestValue {
    match layout {
        Layout::Complete => {
            let point = || {
                node(
                    TestInstance::new(POINT)
                        .value("x", TestValue::Single(-0.0))
                        .value("y", TestValue::Single(0.5)),
                )
            };
            node(
                TestInstance::new(CURVE)
                    .value("useLUT", TestValue::Bool(false))
                    .value("points", TestValue::Array(vec![point(), point()])),
            )
        }
        Layout::BrokenCurve => TestValue::Reference(MISSING),
    }
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

const fn node(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}
