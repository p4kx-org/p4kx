// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, Compression, Kind, build_catalog};
use p4kx_extraction::Filter;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_ammo_definitions_fixture.rs"]
mod fixture;
#[path = "catalog_ammo_geometry.rs"]
mod geometry;
#[path = "catalog_ammo_radar_declaration.rs"]
mod radar_declaration;

#[path = "catalog_weapon_magazine_slots.rs"]
mod weapon_magazine_slots;

const PATH: &str = "/definitions/ammoDefinitions";
const NONFINITE_IMPACT_RADIUS: f32 = f32::from_bits(0x7fc0_1234);

#[test]
fn ammo_definitions_share_source_with_legacy_weapons_and_preserve_unlinked_rows() -> TestResult {
    let source = fixture::database(NONFINITE_IMPACT_RADIUS);
    let (_folder, archive) = common::open(&source, DcbOrigin::Encrypted)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let [bullet, tachyon] = proof::rows(&catalog, &format!("{PATH}/rows"))? else {
        return Err("one selected row for each physical Ammo Record".into());
    };
    check_bullet(&catalog, bullet)?;
    check_tachyon(tachyon)?;
    check_weapons(&catalog)?;
    assert_eq!(
        proof::at(&catalog, "/exportHealth/status")?,
        "complete",
        "the emitted Ammo raw receipt covers the same selected and dependent physical read"
    );
    assert_eq!(
        proof::at(
            kind(&catalog, "ammoDefinitions")?,
            "/sourceRecords/selected"
        )?,
        &json!(2),
        "standalone ammunition selection remains independent"
    );
    assert_eq!(
        proof::at(kind(&catalog, "items")?, "/sourceRecords/dependencies")?,
        &json!(2),
        "one actual Ammo dependency and one actual magazine dependency"
    );
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    validator
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    proof::closure(&catalog)
}

#[test]
fn ammo_definitions_round_trip_through_the_deterministic_catalog_writer() -> TestResult {
    let source = fixture::database(NONFINITE_IMPACT_RADIUS);
    let (_folder, archive) = common::open(&source, DcbOrigin::Encrypted)?;
    let options = common::options();
    let (plain_summary, plain) =
        p4kx_export::write_catalog(&archive, &options, Compression::None, || Ok(Vec::new()))?;
    let (packed_summary, packed) =
        p4kx_export::write_catalog(&archive, &options, Compression::ZSTD, || Ok(Vec::new()))?;
    let (_, repeated) =
        p4kx_export::write_catalog(&archive, &options, Compression::ZSTD, || Ok(Vec::new()))?;
    assert!(
        packed == repeated,
        "repeated catalog writes produce identical Zstd frames"
    );

    let decoded = zstd::stream::decode_all(packed.as_slice())?;
    assert!(
        decoded == plain,
        "the Zstd frame decodes to the exact plain catalog bytes"
    );
    assert_eq!(
        plain_summary.size(),
        packed_summary.size(),
        "same uncompressed size"
    );
    assert_eq!(
        packed_summary.compressed_size(),
        Some(u64::try_from(packed.len())?),
        "actual compressed size"
    );
    assert_eq!(
        packed_summary
            .kind(Kind::AmmoDefinitions)
            .map(|kind| kind.count),
        Some(2),
        "compression retains the selected physical definition count"
    );
    let catalog: Value = serde_json::from_slice(&decoded)?;
    let [bullet, tachyon] = proof::rows(&catalog, &format!("{PATH}/rows"))? else {
        return Err("both original ammunition rows survive compression".into());
    };
    check_bullet(&catalog, bullet)?;
    check_tachyon(tachyon)?;
    check_weapons(&catalog)?;
    proof::closure(&catalog)
}

#[test]
fn ammo_definitions_respect_selection_while_filtered_dependencies_feed_legacy() -> TestResult {
    let source = fixture::database(0.0);
    let (_folder, archive) = common::open(&source, DcbOrigin::Plain)?;
    let items = Filter::parse("*/entities/scitem/*")?;
    let options = CatalogOptions {
        filter: Some(&items),
        ..common::options()
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    assert!(
        proof::rows(&catalog, &format!("{PATH}/rows"))?.is_empty(),
        "a legacy dependency never auto-selects a definition"
    );
    check_weapons(&catalog)?;
    assert_eq!(
        proof::at(kind(&catalog, "items")?, "/sourceRecords/dependencies")?,
        &json!(2),
        "the filtered projection retained both real dependencies"
    );
    let ammo = Filter::parse("*/ammoparams/*")?;
    let options = CatalogOptions {
        filter: Some(&ammo),
        ..common::options()
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    assert_eq!(
        proof::rows(&catalog, &format!("{PATH}/rows"))?.len(),
        2,
        "explicit Ammo selection emits definitions without weapons"
    );
    assert!(
        proof::rows(&catalog, "/items")?.is_empty(),
        "no Item selection"
    );
    Ok(())
}

fn check_bullet(catalog: &Value, row: &Value) -> TestResult {
    proof::present(
        row,
        "guid",
        &json!(common::guid(fixture::BULLET)),
        Some(true),
    )?;
    proof::present(row, "recordIndex", &json!(0), Some(true))?;
    proof::present(row, "sourceType", &json!("AmmoParams"), Some(true))?;
    proof::present(row, "size", &json!(3), Some(true))?;
    let projectile = proof::at(row, "/projectileParams/value")?;
    assert_eq!(
        proof::at(projectile, "/kind/type")?,
        "bullet",
        "original projectile kind"
    );
    let impact = proof::present(
        proof::at(projectile, "/kind/value")?,
        "impactRadius",
        &json!({"rawBits":"7fc01234","finite":null}),
        Some(true),
    )?;
    assert!(
        proof::uses(catalog, impact)?.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some("payload")
                && used.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
                && used.pointer("/location/indices/0").and_then(Value::as_str) == Some("0")
                && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
        }),
        "the emitted nonfinite word cites the actual Ammo Record field"
    );
    let conversion = proof::at(row, "/conversionRate/value")?;
    assert_eq!(
        proof::at(conversion, "/type")?,
        "centi",
        "original cargo unit variant"
    );
    proof::present(conversion, "centiScu", &json!(-7), Some(true))?;
    Ok(())
}

fn check_tachyon(row: &Value) -> TestResult {
    proof::present(
        row,
        "guid",
        &json!(common::guid(fixture::TACHYON)),
        Some(true),
    )?;
    proof::present(row, "recordIndex", &json!(1), Some(true))?;
    let projectile = proof::at(row, "/projectileParams/value/kind")?;
    assert_eq!(
        proof::at(projectile, "/type")?,
        "tachyon",
        "unlinked authored variant"
    );
    proof::present(
        proof::at(projectile, "/value")?,
        "zeroDamageRange",
        &json!({"rawBits":"40000000","finite":2.0}),
        Some(true),
    )?;
    Ok(())
}

fn check_weapons(catalog: &Value) -> TestResult {
    let direct = weapon(catalog, fixture::DIRECT)?;
    let personal = weapon(catalog, fixture::PERSONAL)?;
    let first = proof::at(direct, "/ammo")?;
    let second = proof::at(personal, "/ammo")?;
    assert_eq!(
        first, second,
        "both legacy routes reach the same physical ammunition"
    );
    assert_eq!(
        proof::at(first, "/speed")?,
        &json!(12.5),
        "previous primary speed"
    );
    assert_eq!(
        proof::at(first, "/lifetime")?,
        &json!(0.0),
        "previous primary lifetime"
    );
    assert_eq!(
        proof::at(direct, "/ammoCapacity")?,
        &json!(200),
        "direct capacity"
    );
    assert_eq!(
        proof::at(personal, "/ammoCapacity")?,
        &json!(30),
        "magazine capacity"
    );
    Ok(())
}

fn weapon(catalog: &Value, guid: u128) -> TestResult<&Value> {
    let expected = json!(common::guid(guid));
    let row = proof::rows(catalog, "/items")?
        .iter()
        .find(|row| row.get("guid") == Some(&expected))
        .ok_or("selected weapon Item")?;
    proof::at(row, "/stats/weapon")
}

fn kind<'catalog>(catalog: &'catalog Value, name: &str) -> TestResult<&'catalog Value> {
    proof::rows(catalog, "/exportHealth/kinds")?
        .iter()
        .find(|kind| kind.get("kind").and_then(Value::as_str) == Some(name))
        .ok_or_else(|| format!("missing health kind {name}").into())
}
