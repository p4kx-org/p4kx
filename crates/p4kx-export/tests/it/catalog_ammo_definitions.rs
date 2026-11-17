// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::canonical_weapons::{ammo_definition, primary_ammo};
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
#[path = "catalog_ammo_rigid.rs"]
mod rigid;

#[path = "catalog_ammo_filter_dependencies.rs"]
mod filter_dependencies;

#[path = "catalog_weapon_magazine_slots.rs"]
mod weapon_magazine_slots;

const PATH: &str = "/definitions/ammoDefinitions";
const NONFINITE_IMPACT_RADIUS: f32 = f32::from_bits(0x7fc0_1234);

#[test]
fn ammo_definitions_share_source_with_weapon_routes_and_preserve_unlinked_rows() -> TestResult {
    let source = fixture::database(NONFINITE_IMPACT_RADIUS);
    let (_folder, archive) = common::open(&source, DcbOrigin::Encrypted)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let [bullet, tachyon] = proof::rows(&catalog, &format!("{PATH}/rows"))? else {
        return Err("one selected row for each physical Ammo Record".into());
    };
    check_bullet(&catalog, bullet)?;
    check_tachyon(tachyon)?;
    check_weapons(&catalog, true)?;
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
    check_weapons(&catalog, true)?;
    proof::closure(&catalog)
}

#[test]
fn ammo_definition_rows_distinguish_filtered_dependencies_from_explicit_selection() -> TestResult {
    let source = fixture::database(0.0);
    let (_folder, archive) = common::open(&source, DcbOrigin::Plain)?;
    let items = Filter::parse("*/entities/scitem/*")?;
    let options = CatalogOptions {
        filter: Some(&items),
        ..common::options()
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let [ammo] = proof::rows(&catalog, &format!("{PATH}/rows"))? else {
        return Err("one exact dependency Ammo Record".into());
    };
    assert_eq!(
        proof::at(ammo, "/selection")?,
        "dependency",
        "a required definition retains its selection role"
    );
    assert_eq!(
        proof::at(
            kind(&catalog, "ammoDefinitions")?,
            "/sourceRecords/selected"
        )?,
        &json!(0),
        "emitting a dependency does not select it"
    );
    check_weapons(&catalog, false)?;
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
    let rows = proof::rows(&catalog, &format!("{PATH}/rows"))?;
    assert_eq!(
        rows.len(),
        2,
        "explicit Ammo selection emits definitions without weapons"
    );
    for row in rows {
        assert_eq!(
            proof::at(row, "/selection")?,
            "selected",
            "ordinary definitions carry their selection role"
        );
    }
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

fn check_weapons(catalog: &Value, encrypted: bool) -> TestResult {
    let direct = primary_ammo(catalog, fixture::DIRECT, "ownContainer", 200, encrypted)?;
    let linked = primary_ammo(catalog, fixture::PERSONAL, "linkedContainer", 30, encrypted)?;
    assert_eq!(
        proof::at(direct, "/targetRecordIndex/value")?,
        proof::at(linked, "/targetRecordIndex/value")?,
        "both routes reach the same physical ammunition"
    );
    let definition = ammo_definition(catalog, direct)?;
    for (field, value) in [
        ("speed", json!({"rawBits":"41480000", "finite":12.5})),
        ("lifetime", json!({"rawBits":"00000000", "finite":0.0})),
    ] {
        proof::present(definition, field, &value, Some(encrypted))?;
    }
    Ok(())
}

fn kind<'catalog>(catalog: &'catalog Value, name: &str) -> TestResult<&'catalog Value> {
    proof::rows(catalog, "/exportHealth/kinds")?
        .iter()
        .find(|kind| kind.get("kind").and_then(Value::as_str) == Some(name))
        .ok_or_else(|| format!("missing health kind {name}").into())
}
