// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, test_key,
};
use serde_json::{Value, json};

use super::ReducedError;
use super::ammo_definitions;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const AMMO: &str = "AmmoParams";
const PROJECTILE: &str = "ProjectileParams";
const BULLET: &str = "BulletProjectileParams";
const TACHYON: &str = "TachyonProjectileParams";
const COUNTERMEASURE: &str = "CounterMeasureProjectileParams";
const DAMAGE: &str = "DamageBase";
const INFO: &str = "DamageInfo";
const PARAMETERS: &str = "DamageParams";
const OWNER: u128 = 0x1177_0300;
const INFO_POINTER: &str = "/projectileParams/value/kind/value/damage/value";

#[derive(Clone, Copy)]
enum Layout {
    Complete,
    InvalidDamage,
}

#[test]
fn retains_bullet_and_tachyon_damage_by_physical_ammo_record() -> TestResult {
    with_queries(Layout::Complete, |parsed, scratch| {
        let (table, targets) = ammo_definitions::collect(parsed, scratch, None)?;
        let wire = serde_json::to_string(&table)?;
        let json: Value = serde_json::from_str(&wire)?;
        let rows = at(&json, "/rows")?.as_array().ok_or("ammunition rows")?;
        let [bullet, tachyon, remainder @ ..] = rows.as_slice() else {
            return Err("physical bullet and tachyon aliases".into());
        };
        let mut positions = targets
            .iter()
            .map(|(position, _)| position)
            .collect::<Vec<_>>();
        positions.sort_unstable();
        assert_eq!(positions, [0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(bullet.pointer("/recordIndex/value"), Some(&json!(0)));
        assert_eq!(tachyon.pointer("/recordIndex/value"), Some(&json!(1)));
        assert_eq!(bullet.get("guid"), tachyon.get("guid"), "same nominal GUID");
        assert_ne!(
            bullet.get("recordPath"),
            tachyon.get("recordPath"),
            "physical paths"
        );
        assert_damage_channels(bullet, "bullet", 3.5)?;
        assert_damage_channels(tachyon, "tachyon", 4.5)?;
        assert_non_damage_forms(remainder)?;
        for unrelated in [
            "speed",
            "damageTotal",
            "lifetime",
            "impactRadius",
            "sourceUsesRefs",
            "rawBits",
        ] {
            assert!(!wire.contains(unrelated), "excluded key {unrelated}");
        }
        Ok(())
    })
}

#[test]
fn malformed_selected_ammo_damage_refuses_the_reduced_slice() -> TestResult {
    with_queries(Layout::InvalidDamage, |parsed, scratch| {
        let error = ammo_definitions::collect(parsed, scratch, None).err();
        assert!(
            matches!(
                error,
                Some(ReducedError::IncompleteScope(ammo_definitions::PATH))
            ),
            "incompatible authored damage storage cannot become zero or absence"
        );
        Ok(())
    })
}

fn assert_damage_channels(row: &Value, kind: &str, physical: f32) -> TestResult {
    assert_eq!(
        row.pointer("/projectileParams/value/kind/type"),
        Some(&json!(kind))
    );
    let damage = at(row, INFO_POINTER)?;
    assert_eq!(damage.get("type"), Some(&json!("info")));
    for (channel, expected) in [
        ("physical", json!(physical)),
        ("energy", json!(-0.0)),
        ("distortion", Value::Null),
        ("thermal", json!(0.0)),
        ("biochemical", json!(1.25)),
        ("stun", json!(8.0)),
    ] {
        let word = at(damage, "/value")?
            .get(channel)
            .ok_or("original damage channel")?;
        assert_eq!(word.get("state"), Some(&json!("present")), "{channel}");
        assert_eq!(
            word.get("containsEncryptedSource"),
            Some(&json!(true)),
            "{channel}"
        );
        assert_eq!(
            word.pointer("/value/type"),
            Some(&json!("f32")),
            "{channel}"
        );
        assert_eq!(word.pointer("/value/finite"), Some(&expected), "{channel}");
    }
    assert_eq!(
        damage
            .pointer("/value/energy/value/finite")
            .and_then(Value::as_f64)
            .map(f64::to_bits),
        Some((-0.0_f64).to_bits())
    );
    assert_eq!(
        damage.pointer("/value/distortion/value/nonFinite"),
        Some(&json!("positiveInfinity"))
    );
    Ok(())
}

fn assert_non_damage_forms(rows: &[Value]) -> TestResult {
    let [parameters, base, countermeasure, empty, base_damage] = rows else {
        return Err("all authored non-channel cases".into());
    };
    assert_eq!(at(parameters, INFO_POINTER)?, &json!({"type":"parameters"}));
    assert_eq!(
        base.pointer("/projectileParams/value"),
        Some(&json!({"kind":{"type":"base"}}))
    );
    assert_eq!(
        countermeasure.pointer("/projectileParams/value"),
        Some(&json!({"kind":{"type":"countermeasure"}}))
    );
    assert_eq!(
        empty.pointer("/projectileParams/state"),
        Some(&json!("null"))
    );
    assert_eq!(at(base_damage, INFO_POINTER)?, &json!({"type":"base"}));
    Ok(())
}

fn at<'read>(row: &'read Value, pointer: &str) -> TestResult<&'read Value> {
    row.pointer(pointer)
        .ok_or_else(|| format!("missing authored ammo field {pointer}").into())
}

fn with_queries(
    layout: Layout,
    inspect: impl FnOnce(&ParsedCatalogQueries<'_>, &Path) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database(layout).build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    inspect(&session.parsed(None)?, scratch.path())
}

fn database(layout: Layout) -> DataForgeBuilder {
    let damage_storage = match layout {
        Layout::Complete => pointer(DAMAGE),
        Layout::InvalidDamage => TestType::Int32,
    };
    let mut structure = TestStruct::new(INFO).parent(DAMAGE);
    for (name, _) in damage_words(0.0) {
        structure = structure.field(name, TestType::Single);
    }
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(AMMO)
                .field("projectileParams", pointer(PROJECTILE))
                .field("speed", TestType::Single),
        )
        .structure(TestStruct::new(PROJECTILE))
        .structure(
            TestStruct::new(BULLET)
                .parent(PROJECTILE)
                .field("damage", damage_storage),
        )
        .structure(
            TestStruct::new(TACHYON)
                .parent(PROJECTILE)
                .field("damage", pointer(DAMAGE)),
        )
        .structure(TestStruct::new(COUNTERMEASURE).parent(PROJECTILE))
        .structure(TestStruct::new(DAMAGE))
        .structure(structure)
        .structure(
            TestStruct::new(PARAMETERS)
                .parent(DAMAGE)
                .field("damageTotal", TestType::Single),
        );
    match layout {
        Layout::Complete => populate(builder),
        Layout::InvalidDamage => builder.record(ammo(
            OWNER,
            "invalid_damage",
            projectile(BULLET, TestValue::Int32(4)),
        )),
    }
}

fn populate(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .record(ammo(OWNER, "fixture_bullet", projectile(BULLET, info(3.5))))
        .record(ammo(OWNER, "fixture_alias", projectile(TACHYON, info(4.5))))
        .record(ammo(
            OWNER + 1,
            "fixture_parameters",
            projectile(
                BULLET,
                instance(
                    TestInstance::new(PARAMETERS).value("damageTotal", TestValue::Single(15.0)),
                ),
            ),
        ))
        .record(ammo(
            OWNER + 2,
            "fixture_base",
            instance(TestInstance::new(PROJECTILE)),
        ))
        .record(ammo(
            OWNER + 3,
            "fixture_countermeasure",
            instance(TestInstance::new(COUNTERMEASURE)),
        ))
        .record(ammo(OWNER + 4, "fixture_empty", TestValue::Null))
        .record(ammo(
            OWNER + 5,
            "fixture_base_damage",
            projectile(BULLET, instance(TestInstance::new(DAMAGE))),
        ))
}

fn ammo(owner: u128, name: &str, projectile: TestValue) -> TestRecord {
    TestRecord::new(
        owner,
        AMMO,
        format!("{AMMO}.{name}"),
        format!("libs/foundry/records/ammoparams/fixture/{name}.xml"),
    )
    .value("projectileParams", projectile)
    .value("speed", TestValue::Single(12.5))
}

fn projectile(class: &str, damage: TestValue) -> TestValue {
    instance(TestInstance::new(class).value("damage", damage))
}

fn info(physical: f32) -> TestValue {
    let fields = damage_words(physical)
        .into_iter()
        .fold(TestInstance::new(INFO), |instance, (field, amount)| {
            instance.value(field, TestValue::Single(amount))
        });
    instance(fields)
}

const fn damage_words(physical: f32) -> [(&'static str, f32); 6] {
    [
        ("DamagePhysical", physical),
        ("DamageEnergy", -0.0),
        ("DamageDistortion", f32::INFINITY),
        ("DamageThermal", 0.0),
        ("DamageBiochemical", 1.25),
        ("DamageStun", 8.0),
    ]
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

const fn instance(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}
