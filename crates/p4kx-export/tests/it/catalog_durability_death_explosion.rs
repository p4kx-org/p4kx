// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForge, DataType, StructValue, Value as Original, ValueError};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use super::{DURABILITY, good_database};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_death_explosion_fixture.rs"]
mod fixture;

#[path = "catalog_explosion_definitions.rs"]
mod definitions;

const FIELD: &str = "deathExplosionParams";
const ORIGINAL_FIELD: &str = "DeathExplosionParams";

#[test]
fn durability_death_explosion_keeps_authored_forms_and_exact_origin() -> TestResult {
    let source = fixture::database(false);
    check_original(&source)?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&source, origin)?;
        let mut options = common::options();
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
        let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
        let [inline, base, linked, dangling, wrong, null] =
            proof::rows(&catalog, &format!("{DURABILITY}/rows"))?
        else {
            return Err("one row per actual selected Health occurrence".into());
        };
        check_inline(&catalog, inline, encrypted)?;
        proof::present(base, FIELD, &json!({"type": "base"}), Some(encrypted))?;
        check_link(&catalog, linked, encrypted)?;
        check_failed_link(&catalog, dangling, fixture::MISSING, "dangling", encrypted)?;
        check_wrong_target(&catalog, wrong, encrypted)?;
        let null = proof::cell(null, FIELD, "null")?;
        assert_eq!(
            null["containsEncryptedSource"], encrypted,
            "an authored null retains its physical marker"
        );
        require_use(&catalog, null, "$value", "selector")?;
    }
    Ok(())
}

#[test]
fn durability_death_explosion_distinguishes_absence_and_unknown_concrete_type() -> TestResult {
    let (_, absent) = common::build(&good_database())?;
    let first = proof::at(&absent, "/configurations/durability/rows/0")?;
    let cell = proof::cell(first, FIELD, "absent")?;
    require_use(&absent, cell, "$fields", "selector")?;

    let source = fixture::database(true);
    let original = source.build();
    let forge = DataForge::open(&original)?;
    let tree = forge.record(0).ok_or("physical owner")?.tree()?;
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("original Health components".into());
    };
    assert_eq!(
        original_owned(components.last().ok_or("last original component")?)?.type_name(),
        "FixtureExplosionUnknown",
        "selected source really contains an unsupported derived type"
    );
    let (_folder, archive) = common::open(&source, DcbOrigin::Encrypted)?;
    assert!(
        matches!(
            build_catalog(&archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "the unknown original shape is a projection loss"
    );
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let rows = proof::rows(&catalog, &format!("{DURABILITY}/rows"))?;
    let unknown = proof::cell(
        rows.last().ok_or("last diagnostic Health occurrence")?,
        FIELD,
        "unsupported",
    )?;
    require_use(&catalog, unknown, "$type", "selector")?;
    let diagnostic = proof::rows(&catalog, "/exportHealth/diagnostics")?
        .iter()
        .find(|row| row["code"] == "unsupportedValue")
        .ok_or("the actual unsupported source shape is diagnosed")?;
    let [example] = proof::rows(diagnostic, "/examples")? else {
        return Err("one unsupported explosion occurrence".into());
    };
    assert_eq!(
        proof::example_text(&catalog, example, "field")?,
        Some(&json!("Components[7].DeathExplosionParams")),
        "the diagnostic identifies the actual unknown explosion slot"
    );
    Ok(())
}

fn check_original(source: &DataForgeBuilder) -> TestResult {
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    check_declaration(&forge)?;
    let tree = forge.record(0).ok_or("physical owner")?.tree()?;
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("original Health components".into());
    };
    let [_, inline, base, linked, missing, wrong, null] = components.as_ref() else {
        return Err("actual ordered Health component slots".into());
    };
    let blast = original_owned(inline)?;
    assert_eq!(blast.type_name(), "ExplosionParams", "physical inline type");
    assert_eq!(
        original_owned(base)?.type_name(),
        "ExplosionBaseParams",
        "physical fieldless type"
    );
    let Some(Original::Reference(link)) = original_owned(linked)?.field("record") else {
        return Err("physically resolved ExplosionRecord reference".into());
    };
    assert_eq!(
        link.guid().to_string(),
        common::guid(fixture::TARGET),
        "exact authored GUID"
    );
    assert_eq!(
        link.type_name(),
        "ExplosionRecord",
        "declared compatible target"
    );
    assert!(
        link.location().is_none(),
        "the reference addresses a whole Record"
    );
    check_original_failed_links(original_owned(missing)?, original_owned(wrong)?);
    assert!(
        matches!(
            original_health(null)?.field(ORIGINAL_FIELD),
            Some(Original::Null)
        ),
        "the final actual pointer is null"
    );
    Ok(())
}

fn check_declaration(forge: &DataForge<'_>) -> TestResult {
    let health = forge
        .structs()
        .find(|structure| structure.name() == Ok("SHealthComponentParams"))
        .ok_or("physical Health type")?;
    let declaration = health
        .properties()
        .find(|field| field.name() == Ok(ORIGINAL_FIELD))
        .ok_or("authored DeathExplosion declaration")?;
    assert_eq!(
        declaration.data_type()?,
        DataType::StrongPointer,
        "authored storage is a strong pointer"
    );
    assert_eq!(
        declaration
            .target_struct()
            .ok_or("declared Explosion base")?
            .name()?,
        "ExplosionBaseParams",
        "the authored field carries concrete alternatives"
    );
    Ok(())
}

fn check_original_failed_links(missing: &StructValue<'_>, wrong: &StructValue<'_>) {
    assert!(
        matches!(missing.field("record"), Some(Original::Error(ValueError::DanglingReference { guid }))
            if guid.to_string() == common::guid(fixture::MISSING)),
        "physical dangling Reference"
    );
    assert!(
        matches!(wrong.field("record"), Some(Original::Reference(target))
            if target.guid().to_string() == common::guid(fixture::WRONG)
                && target.type_name() == "FixtureWrongTarget"
                && target.record_index() == 2),
        "the physical canonical Reference exposes the incompatible target header"
    );
}

fn original_health<'read, 'source>(
    component: &'read Original<'source>,
) -> TestResult<&'read StructValue<'source>> {
    let Original::Struct(health) = component else {
        return Err("original Health component".into());
    };
    Ok(health)
}

fn original_owned<'read, 'source>(
    component: &'read Original<'source>,
) -> TestResult<&'read StructValue<'source>> {
    let Some(Original::Struct(explosion)) = original_health(component)?.field(ORIGINAL_FIELD)
    else {
        return Err("original owned explosion occurrence".into());
    };
    Ok(explosion)
}

fn check_inline(catalog: &Value, row: &Value, encrypted: bool) -> TestResult {
    let cell = proof::cell(row, FIELD, "present")?;
    assert_eq!(
        cell["containsEncryptedSource"], encrypted,
        "actual component encryption"
    );
    require_use(catalog, cell, "$type", "selector")?;
    assert_eq!(
        proof::at(cell, "/value/type")?,
        "inline",
        "actual owned inline alternative"
    );
    let blast = proof::at(cell, "/value/value")?;
    let flag = proof::present(blast, "useRandomScale", &json!(false), Some(encrypted))?;
    require_use(catalog, flag, "$value", "payload")?;
    proof::present(
        blast,
        "friendlyFire",
        &json!("FixtureRule"),
        Some(encrypted),
    )?;
    proof::present(
        blast,
        "minRadius",
        &float("00000000", Some(0.0)),
        Some(encrypted),
    )?;
    proof::present(blast, "pressure", &float("7f800000", None), Some(encrypted))?;
    proof::present(
        proof::at(blast, "/explosionFront/value")?,
        "x",
        &float("bf800000", Some(-1.0)),
        Some(encrypted),
    )?;
    check_inline_children(blast, encrypted)?;
    proof::cell(blast, "flashbangParams", "null")?;
    Ok(())
}

fn check_inline_children(blast: &Value, encrypted: bool) -> TestResult {
    proof::present(
        proof::at(blast, "/damage/value/value")?,
        "physical",
        &float("3f800000", Some(1.0)),
        Some(encrypted),
    )?;
    proof::present(
        proof::at(blast, "/damage/value/value")?,
        "energy",
        &float("40000000", Some(2.0)),
        Some(encrypted),
    )?;
    proof::present(
        proof::at(blast, "/fragmentParams/value")?,
        "amount",
        &json!(-2),
        Some(encrypted),
    )?;
    proof::present(
        proof::at(blast, "/shockwaveParams/value")?,
        "primitiveType",
        &json!("FixtureShock"),
        Some(encrypted),
    )?;
    proof::present(
        proof::at(blast, "/effect/value")?,
        "path",
        &json!("fixture_particle"),
        Some(encrypted),
    )?;
    Ok(())
}

fn check_link(catalog: &Value, row: &Value, encrypted: bool) -> TestResult {
    let carrier = proof::cell(row, FIELD, "present")?;
    assert_eq!(
        proof::at(carrier, "/value/type")?,
        "byRecord",
        "the link is a separate authored form"
    );
    let record = proof::at(carrier, "/value/record/value")?;
    assert_eq!(record["status"], "present", "the actual link resolves");
    let guid = proof::present(
        record,
        "guid",
        &json!(common::guid(fixture::TARGET)),
        Some(encrypted),
    )?;
    require_use(catalog, guid, "$value", "payload")?;
    let target = proof::present(
        record,
        "targetType",
        &json!("ExplosionRecord"),
        Some(encrypted),
    )?;
    assert!(
        proof::uses(catalog, target)?.iter().any(|used| {
            has_location(used, "dataforgeRecordHeader", "sourceType")
                && used.pointer("/location/indices") == Some(&json!(["1"]))
        }),
        "the selected target's actual header contributes"
    );
    proof::present(record, "targetRecordIndex", &json!(1), Some(encrypted))?;
    Ok(())
}

fn check_failed_link(
    catalog: &Value,
    row: &Value,
    guid: u128,
    status: &str,
    encrypted: bool,
) -> TestResult {
    let target = proof::at(row, "/deathExplosionParams/value/record/value")?;
    assert_eq!(
        target["status"], status,
        "the original link failure remains typed"
    );
    let guid = proof::present(target, "guid", &json!(common::guid(guid)), Some(encrypted))?;
    require_use(catalog, guid, "$error.guid", "payload")?;
    proof::cell(target, "targetRecordIndex", "absent")?;
    Ok(())
}

fn check_wrong_target(catalog: &Value, row: &Value, encrypted: bool) -> TestResult {
    let carrier = proof::at(row, "/deathExplosionParams/value/record")?;
    let target = proof::at(carrier, "/value")?;
    assert_eq!(
        target["status"], "unexpectedTargetType",
        "actual target fails the declared Reference type"
    );
    let guid = proof::present(
        target,
        "guid",
        &json!(common::guid(fixture::WRONG)),
        Some(encrypted),
    )?;
    require_use(catalog, guid, "$value", "payload")?;
    proof::present(target, "targetRecordIndex", &json!(2), Some(encrypted))?;
    proof::present(
        target,
        "targetType",
        &json!("FixtureWrongTarget"),
        Some(encrypted),
    )?;
    require_use(catalog, carrier, "$target", "selector")?;
    let uses = proof::uses(catalog, carrier)?;
    assert!(
        uses.iter().any(
            |used| has_location(used, "dataforgeRecordHeader", "sourceType")
                && used.pointer("/location/indices") == Some(&json!(["2"]))
                && used["role"] == "selector"
        ),
        "status consults the actual incompatible target type"
    );
    assert!(
        uses.iter().any(
            |used| has_location(used, "dataforgeStructDeclaration", "parent")
                && used["role"] == "selector"
        ),
        "the inspected ancestry contributes to the typed mismatch"
    );
    Ok(())
}

fn require_use(catalog: &Value, carrier: &Value, field: &str, role: &str) -> TestResult {
    assert!(
        proof::uses(catalog, carrier)?.iter().any(|used| {
            has_location(used, "dataforgeValue", field)
                && used.pointer("/location/indices/0").and_then(Value::as_str) == Some("0")
                && used["role"] == role
        }),
        "the actual owner contributes {field} as {role}"
    );
    Ok(())
}

fn has_location(used: &Value, namespace: &str, field: &str) -> bool {
    used.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
        && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
}

fn float(raw: &str, finite: Option<f32>) -> Value {
    json!({"rawBits": raw, "finite": finite})
}
