// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestRecord, TestStruct,
    TestType, TestValue, entity_schema, test_key,
};
use serde_json::{Value, json};

use super::ReducedError;
use super::vehicle_armor;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const ARMOR: &str = "SCItemVehicleArmorParams";
const BASE: &str = "DamageBase";
const INFO: &str = "DamageInfo";
const PARAMETERS: &str = "DamageParams";
const MACRO: &str = "DamageMacro";
const FUTURE: &str = "FixtureFutureDamage";
const PENETRATION: &str = "ArmorPenetrationResistanceParams";
const DEFLECTION: &str = "ArmorDeflectionParams";
const CURVE: &str = "BezierCurve";
const POINT: &str = "Vec2";
const OWNER: u128 = 0x1177_0100;
const MACRO_GUID: u128 = 0x1177_0110;
const MISSING: u128 = 0x1177_0111;

#[derive(Clone, Copy)]
enum CurveLayout {
    Inline,
    BrokenReference,
}

#[test]
fn preserves_physical_vehicle_armor_cells_and_original_damage_links() -> TestResult {
    with_queries(CurveLayout::Inline, |queries, scratch| {
        let output = vehicle_armor::collect(queries, scratch, None)?;
        let encoded = serde_json::to_string(&output)?;
        let table: Value = serde_json::from_str(&encoded)?;
        let rows = table
            .get("rows")
            .and_then(Value::as_array)
            .ok_or("armor rows")?;
        let [armor, resolved, dangling, future, empty] = rows.as_slice() else {
            return Err("one row per physical vehicle armor occurrence".into());
        };

        assert_eq!(armor.get("ownerGuid"), resolved.get("ownerGuid"));
        assert_eq!(armor.pointer("/ownerRecordIndex/value"), Some(&json!(0)));
        assert_eq!(resolved.pointer("/ownerRecordIndex/value"), Some(&json!(1)));
        assert_eq!(armor.pointer("/componentOrdinal/value"), Some(&json!(1)));
        assert_eq!(resolved.pointer("/componentOrdinal/value"), Some(&json!(0)));
        assert_numeric_cells(armor)?;
        assert_macro_link(resolved, "present", Some(5))?;
        assert_macro_link(dangling, "dangling", None)?;
        assert_eq!(
            future.pointer("/damageMultiplier/value/sourceType/value"),
            Some(&json!(FUTURE))
        );
        assert_eq!(
            empty.pointer("/damageMultiplier/state"),
            Some(&json!("null"))
        );
        assert!(!encoded.contains("sourceUsesRefs"));
        assert!(!encoded.contains("rawBits"));
        assert!(!encoded.contains("additionalFields"));
        assert!(!encoded.contains("fixtureExtra"));
        Ok(())
    })
}

#[test]
fn damaged_selected_health_curve_refuses_vehicle_armor_slice() -> TestResult {
    with_queries(CurveLayout::BrokenReference, |queries, scratch| {
        let error = vehicle_armor::collect(queries, scratch, None).err();
        assert!(
            matches!(
                error,
                Some(ReducedError::IncompleteScope(vehicle_armor::PATH))
            ),
            "a present but malformed selected curve cannot be silently omitted"
        );
        Ok(())
    })
}

fn assert_numeric_cells(armor: &Value) -> TestResult {
    for (pointer, expected) in [
        ("/signalInfrared", Value::Null),
        ("/signalCrossSection", json!(3.5)),
        ("/damageMultiplier/value/physical", json!(4.0)),
        ("/damageMultiplier/value/stun", json!(9.0)),
        (
            "/armorPenetrationResistance/value/basePenetrationReduction",
            json!(0.75),
        ),
        (
            "/armorPenetrationResistance/value/penetrationAbsorptionForType/value/energy",
            json!(15.0),
        ),
        (
            "/armorDeflection/value/deflectionValue/value/thermal",
            json!(27.0),
        ),
        (
            "/armorDeflection/value/healthCurve/value/points/value/0/value/y",
            json!(0.25),
        ),
        (
            "/armorDeflection/value/healthCurve/value/points/value/1/value/x",
            json!(1.0),
        ),
    ] {
        let cell = armor.pointer(pointer).ok_or("selected armor field")?;
        assert_eq!(
            cell.get("containsEncryptedSource"),
            Some(&json!(true)),
            "{pointer}"
        );
        assert_eq!(cell.pointer("/value/finite"), Some(&expected), "{pointer}");
    }
    assert_eq!(
        armor.pointer("/signalInfrared/value/nonFinite"),
        Some(&json!("negativeInfinity"))
    );
    assert_eq!(
        armor.pointer("/armorDeflection/value/healthCurve/value/useLut/value"),
        Some(&json!(true))
    );
    Ok(())
}

fn assert_macro_link(row: &Value, status: &str, expected_target: Option<u32>) -> TestResult {
    let damage = row
        .pointer("/damageMultiplier/value")
        .ok_or("damage parameters")?;
    assert_eq!(damage.get("kind"), Some(&json!("parameters")));
    assert_eq!(
        damage.pointer("/damageTotal/value/finite"),
        Some(&json!(8.0))
    );
    let link = damage
        .pointer("/damageMacro/value")
        .ok_or("authored macro link")?;
    assert_eq!(link.get("type"), Some(&json!("recordReference")));
    assert_eq!(link.pointer("/value/status"), Some(&json!(status)));
    assert_eq!(
        link.pointer("/value/targetRecordIndex/value"),
        Some(&json!(expected_target))
    );
    assert_eq!(
        link.pointer("/value/guid/containsEncryptedSource"),
        Some(&json!(true))
    );
    Ok(())
}

fn with_queries(
    layout: CurveLayout,
    inspect: impl FnOnce(&ParsedCatalogQueries<'_>, &Path) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    let builder = entity_schema(DataForgeBuilder::new().version(8));
    let database = armor_schema(damage_schema(builder), layout);
    let database = armor_records(database, layout);
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    inspect(&session.parsed(None)?, scratch.path())
}

fn armor_records(database: DataForgeBuilder, layout: CurveLayout) -> DataForgeBuilder {
    let direct = TestEntity::new(
        OWNER,
        "FixtureArmor",
        "libs/foundry/records/entities/scitem/ships/armor/fixture_armor.xml",
    )
    .attach_type("Misc")
    .component(armor(layout, damage(0.0)))
    .record();
    database
        .record(direct)
        .record(item(OWNER, "alias", layout, parameters(MACRO_GUID)))
        .record(item(OWNER + 1, "dangling", layout, parameters(MISSING)))
        .record(item(
            OWNER + 2,
            "future",
            layout,
            instance(TestInstance::new(FUTURE)),
        ))
        .record(item(OWNER + 3, "empty", layout, TestValue::Null))
        .record(TestRecord::new(
            MACRO_GUID,
            MACRO,
            "DamageMacro.Fixture",
            "libs/foundry/records/damage/fixture_macro.xml",
        ))
}

fn item(guid: u128, name: &str, layout: CurveLayout, multiplier: TestValue) -> TestRecord {
    TestEntity::new(
        guid,
        name,
        format!("libs/foundry/records/entities/scitem/ships/armor/{name}.xml"),
    )
    .component(armor(layout, multiplier))
    .record()
}

const CHANNELS: [(&str, f32); 6] = [
    ("DamagePhysical", 4.0),
    ("DamageEnergy", 5.0),
    ("DamageDistortion", 6.0),
    ("DamageThermal", 7.0),
    ("DamageBiochemical", 8.0),
    ("DamageStun", 9.0),
];

fn damage_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    let damage = CHANNELS.into_iter().fold(
        TestStruct::new(INFO).parent(BASE),
        |structure, (channel, _)| structure.field(channel, TestType::Single),
    );
    builder
        .structure(TestStruct::new(BASE))
        .structure(damage)
        .structure(TestStruct::new(MACRO))
        .structure(TestStruct::new(FUTURE).parent(BASE))
        .structure(
            TestStruct::new(PARAMETERS)
                .parent(BASE)
                .field("damageMacro", TestType::ReferenceTo(MACRO.to_owned()))
                .field("damageTotal", TestType::Single),
        )
}

fn armor_schema(builder: DataForgeBuilder, layout: CurveLayout) -> DataForgeBuilder {
    let curve_storage = match layout {
        CurveLayout::Inline => class(CURVE),
        CurveLayout::BrokenReference => TestType::ReferenceTo(CURVE.to_owned()),
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
                .field("armorDeflection", class(DEFLECTION))
                .field("fixtureExtra", TestType::Single),
        )
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

fn armor(layout: CurveLayout, multiplier: TestValue) -> TestValue {
    let penetration = instance(
        TestInstance::new(PENETRATION)
            .value("basePenetrationReduction", TestValue::Single(0.75))
            .value("penetrationAbsorptionForType", damage(10.0)),
    );
    let deflection = instance(
        TestInstance::new(DEFLECTION)
            .value("deflectionValue", damage(20.0))
            .value("healthCurve", curve(layout)),
    );
    instance(
        TestInstance::new(ARMOR)
            .value("signalInfrared", TestValue::Single(f32::NEG_INFINITY))
            .value("signalElectromagnetic", TestValue::Single(0.0))
            .value("signalCrossSection", TestValue::Single(3.5))
            .value("damageMultiplier", multiplier)
            .value("armorPenetrationResistance", penetration)
            .value("armorDeflection", deflection)
            .value("fixtureExtra", TestValue::Single(1.25)),
    )
}

fn damage(offset: f32) -> TestValue {
    instance(
        CHANNELS
            .into_iter()
            .fold(TestInstance::new(INFO), |instance, (channel, number)| {
                instance.value(channel, TestValue::Single(number + offset))
            }),
    )
}

fn parameters(guid: u128) -> TestValue {
    instance(
        TestInstance::new(PARAMETERS)
            .value("damageMacro", TestValue::Reference(guid))
            .value("damageTotal", TestValue::Single(8.0)),
    )
}

fn curve(layout: CurveLayout) -> TestValue {
    match layout {
        CurveLayout::Inline => instance(
            TestInstance::new(CURVE)
                .value("useLUT", TestValue::Bool(true))
                .value(
                    "points",
                    TestValue::Array(vec![point(-0.0, 0.25), point(1.0, 0.5)]),
                ),
        ),
        CurveLayout::BrokenReference => TestValue::Reference(MISSING),
    }
}

fn point(x: f32, y: f32) -> TestValue {
    instance(
        TestInstance::new(POINT)
            .value("x", TestValue::Single(x))
            .value("y", TestValue::Single(y)),
    )
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

const fn instance(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}
