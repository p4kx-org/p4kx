// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForge, DataType, Filter, Value as Original};
use serde_json::{Value, json};

use super::fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const PATH: &str = "/definitions/explosionDefinitions";

#[test]
fn explosion_definitions_export_unlinked_records_with_their_own_exact_numeric_proof() -> TestResult
{
    let source = fixture::independent_definitions();
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    for (index, bits) in [(0, 0), (1, 0x7f80_0000)] {
        let tree = forge
            .record(index)
            .ok_or("physical unlinked explosion definition")?
            .tree()?;
        assert_eq!(tree.type_name(), "ExplosionRecord", "actual source type");
        let field = tree
            .root()
            .fields()
            .first()
            .ok_or("original definition field")?;
        assert_eq!(field.name(), "explosionParams", "actual declaration");
        assert_eq!(
            field.storage().data_type(),
            DataType::Class,
            "original owned Class storage"
        );
        let Original::Struct(explosion) = field.value() else {
            return Err("physically readable inline parameters".into());
        };
        assert!(
            matches!(explosion.field("pressure"), Some(Original::Single(raw)) if raw.to_bits() == bits),
            "each physical definition has its exact authored numeric bits"
        );
    }
    let (_folder, archive) = common::open(&source, DcbOrigin::Encrypted)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let definitions = proof::at(&catalog, PATH)?;
    assert_eq!(
        definitions["availability"], "available",
        "declaration and actual Records are available"
    );
    assert_eq!(
        catalog["exportHealth"]["status"], "complete",
        "exact nonfinite definition is covered"
    );
    let [earlier, later] = proof::rows(definitions, "/rows")? else {
        return Err("one definition for each unlinked physical Record".into());
    };
    check_owner(&catalog, earlier, 1, 0x7580, "7f800000", None)?;
    check_owner(&catalog, later, 0, 0x7581, "00000000", Some(0.0))?;
    let kind = proof::rows(&catalog, "/exportHealth/kinds")?
        .iter()
        .find(|kind| kind["kind"] == "explosionDefinitions")
        .ok_or("exact physical definition accounting")?;
    assert_eq!(
        kind["writtenRows"], 2,
        "both physical definitions have been counted"
    );
    assert_eq!(
        kind["sourceRecords"]["selected"], 2,
        "independent selected Record count"
    );
    Ok(())
}

#[test]
fn explosion_definitions_respect_filter_without_erasing_resolved_health_link() -> TestResult {
    let source = fixture::database(false);
    let (_folder, archive) = common::open(&source, DcbOrigin::Plain)?;
    let filter = Filter::parse("*/fixture_death_explosion")?;
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let definitions = proof::at(&catalog, PATH)?;
    assert_eq!(
        definitions["availability"], "available",
        "the filtered family still exists"
    );
    assert!(
        proof::rows(definitions, "/rows")?.is_empty(),
        "a Health dependency does not materialize a filtered definition"
    );
    let link = proof::at(
        &catalog,
        "/configurations/durability/rows/2/deathExplosionParams/value/record/value",
    )?;
    assert_eq!(
        link["status"], "present",
        "the actual authored Record link still resolves"
    );
    proof::present(link, "targetRecordIndex", &json!(1), Some(false))?;
    Ok(())
}

fn check_owner(
    catalog: &Value,
    row: &Value,
    index: u32,
    guid: u128,
    raw_bits: &str,
    finite: Option<f32>,
) -> TestResult {
    proof::present(row, "guid", &json!(common::guid(guid)), Some(true))?;
    proof::present(row, "recordIndex", &json!(index), Some(true))?;
    proof::present(row, "sourceType", &json!("ExplosionRecord"), Some(true))?;
    let params = proof::cell(row, "explosionParams", "present")?;
    let expected = json!({"rawBits": raw_bits, "finite": finite});
    let pressure = proof::present(
        proof::at(params, "/value")?,
        "pressure",
        &expected,
        Some(true),
    )?;
    check_use(catalog, params, index, "$dataType", "selector")?;
    check_use(catalog, pressure, index, "$value", "payload")
}

fn check_use(catalog: &Value, carrier: &Value, index: u32, field: &str, role: &str) -> TestResult {
    let owner = index.to_string();
    assert!(
        proof::uses(catalog, carrier)?.iter().any(|used| {
            used.pointer("/location/namespace").and_then(Value::as_str) == Some("dataforgeValue")
                && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && used.pointer("/location/indices/0").and_then(Value::as_str)
                    == Some(owner.as_str())
                && used["role"] == role
        }),
        "actual definition {index} owns {field} as {role}"
    );
    Ok(())
}
