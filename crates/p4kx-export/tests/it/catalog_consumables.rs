// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{BuiltCatalog, Language, TextOwner, build_catalog, catalog_schema};
use p4kx_extraction::Filter;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_consumable_fixture.rs"]
mod fixture;
#[path = "catalog_consumable_sdk.rs"]
mod sdk;

const CONFIGS: &str = "/configurations/consumables/rows";
const TYPES: &str = "/definitions/consumableTypes/rows";
const SUBTYPES: &str = "/definitions/consumableSubtypes/rows";

#[test]
fn filtered_consumable_keeps_ordered_subtypes_and_raw_volume() -> TestResult {
    let (built, catalog) = catalog("*/consumable/drink")?;
    let [item] = proof::rows(&catalog, CONFIGS)? else {
        return Err("one selected physical component".into());
    };
    proof::present(item, "componentOrdinal", &json!(1), Some(true))?;
    proof::present(item, "ownerGuid", &json!(common::guid(5)), Some(true))?;
    proof::present(item, "containerClosed", &json!(true), Some(true))?;
    proof::present(item, "canBeReclosed", &json!(false), Some(true))?;
    proof::present(item, "containerTypeTag", &json!(""), Some(true))?;
    let volume = proof::at(item, "/consumableVolume/value")?;
    assert_eq!(proof::at(volume, "/kind")?, "microCargoUnit");
    proof::present(volume, "microSCU", &json!(1234), Some(true))?;
    let [first, second] = proof::rows(item, "/defaultContents/value")? else {
        return Err("two slots in original order".into());
    };
    bits(proof::at(first, "/value")?, "ratio", "3e800000")?;
    bits(proof::at(second, "/value")?, "ratio", "3f400000")?;
    let [first_subtype, second_subtype] = proof::rows(&catalog, SUBTYPES)? else {
        return Err("two selected owners' dependencies".into());
    };
    for (link, subtype) in [(first, first_subtype), (second, second_subtype)] {
        assert_eq!(
            proof::at(
                link,
                "/value/consumableSubtype/value/targetRecordIndex/value"
            )?,
            proof::at(subtype, "/recordIndex/value")?,
        );
    }
    assert_eq!(
        proof::rows(&catalog, TYPES)?,
        &[] as &[Value],
        "selection or authored empty list remains empty"
    );
    assert_eq!(
        proof::rows(second_subtype, "/effectsPerMicroSCU/value")?,
        &[] as &[Value],
        "selection or authored empty list remains empty"
    );
    assert!(
        proof::rows(item, "/additionalFields/roots")?
            .iter()
            .any(|row| {
                row.pointer("/name/value").and_then(Value::as_str) == Some("consumeFragmentId")
            })
    );
    check_localization(&built)?;
    proof::closure(&catalog)
}

#[test]
fn original_effects_keep_storage_and_opaque_future_types() -> TestResult {
    let (_, catalog) = catalog("*/consumable/medicine")?;
    let [item] = proof::rows(&catalog, CONFIGS)? else {
        return Err("one medicine component".into());
    };
    proof::present(item, "oneShotConsume", &json!(true), Some(true))?;
    let [subtype] = proof::rows(&catalog, SUBTYPES)? else {
        return Err("one linked subtype".into());
    };
    let slots = proof::rows(subtype, "/effectsPerMicroSCU/value")?;
    let [status, health, null, buff, resource, gas, other] = slots else {
        return Err("all seven original effect slots".into());
    };
    assert_eq!(proof::at(null, "/state")?, "null");
    for (slot, expected) in [
        (status, "modifyActorStatus"),
        (health, "health"),
        (buff, "addBuffEffect"),
        (resource, "resource"),
        (gas, "gas"),
        (other, "authored"),
    ] {
        assert_eq!(proof::at(slot, "/value/kind")?, expected);
    }
    bits(proof::at(status, "/value")?, "statPointChange", "bfc00000")?;
    bits(
        proof::at(status, "/value")?,
        "statCooldownChange",
        "00000000",
    )?;
    bits(proof::at(health, "/value")?, "healthChange", "80000000")?;
    proof::cell(proof::at(health, "/value")?, "effectDescription", "present")?;
    assert_eq!(
        proof::at(buff, "/value/buffDurationOverride/value/sourceType/value")?,
        "BuffDurationOverrideIndefiniteS42",
    );
    bits(
        proof::at(buff, "/value/buffValueOverride/value")?,
        "amount",
        "40400000",
    )?;
    bits(proof::at(resource, "/value")?, "amount", "3e800000")?;
    assert_eq!(
        proof::at(gas, "/value/gasMass/value/gas/value/status")?,
        "dangling"
    );
    assert_eq!(
        proof::at(other, "/value/additionalFields/roots/0/name/value")?,
        "fixtureExtra"
    );
    proof::closure(&catalog)
}

#[test]
fn type_dependencies_and_dangling_items_keep_separate_scopes() -> TestResult {
    let (_, by_type) = catalog("*/type/group")?;
    assert_eq!(
        proof::rows(&by_type, CONFIGS)?,
        &[] as &[Value],
        "selection or authored empty list remains empty"
    );
    let [kind] = proof::rows(&by_type, TYPES)? else {
        return Err("one type row".into());
    };
    assert_eq!(proof::rows(kind, "/subtypes/value")?.len(), 2);
    let [linked, _] = proof::rows(&by_type, SUBTYPES)? else {
        return Err("shared type retains exactly its two physical subtypes".into());
    };
    assert_eq!(
        proof::at(
            linked,
            "/additionalFields/values/0/value/value/value/targetRecordIndex/value"
        )?,
        proof::at(kind, "/recordIndex/value")?,
        "the original back reference resolves to the same physical type without duplicating rows"
    );
    let (_, food) = catalog("*/consumable/food")?;
    assert_eq!(
        proof::rows(&food, SUBTYPES)?,
        &[] as &[Value],
        "selection or authored empty list remains empty"
    );
    let link = proof::at(
        &food,
        "/configurations/consumables/rows/0/defaultContents/value/0/value",
    )?;
    assert_eq!(
        proof::at(link, "/consumableSubtype/value/status")?,
        "dangling"
    );
    bits(link, "ratio", "80000000")?;
    proof::closure(&food)
}

fn bits(row: &Value, field: &str, raw: &str) -> TestResult {
    let cell = proof::cell(row, field, "present")?;
    assert_eq!(
        proof::at(cell, "/value/rawBits")?,
        raw,
        "retain exact Single bits for {field}"
    );
    assert_eq!(
        proof::at(cell, "/containsEncryptedSource")?,
        true,
        "direct origin for {field}"
    );
    Ok(())
}

fn check_localization(built: &BuiltCatalog) -> TestResult {
    let coverage = built.summary().text_kind_coverage();
    assert_eq!(
        coverage.len(),
        Language::ALL.len(),
        "one owner per supported language"
    );
    assert!(
        coverage
            .iter()
            .all(|row| row.owner == TextOwner::ConsumableSubtypes),
        "linked subtype definitions own their registrations"
    );
    let english = coverage
        .iter()
        .find(|row| row.language == Language::English)
        .ok_or("missing English consumable coverage")?;
    assert_eq!(
        english.resolved, 3,
        "two subtypes share one name key and one has a resolved effect description"
    );
    let legacy = built
        .summary()
        .text(Language::English)
        .ok_or("missing English legacy summary")?;
    assert_eq!(
        (legacy.resolved, legacy.unresolved, legacy.placeholder),
        (
            english.resolved,
            english.missing_key + english.empty_source + english.empty_display,
            english.placeholder,
        ),
        "both summaries count each shared lookup occurrence"
    );
    Ok(())
}

fn catalog(filter: &str) -> TestResult<(BuiltCatalog, Value)> {
    let (_temp, archive) = common::open(&fixture::database(), DcbOrigin::Encrypted)?;
    let filter = Filter::new(filter);
    let mut options = common::options();
    options.filter = Some(&filter);
    let catalog = build_catalog(&archive, options.with_full_provenance())?;
    let value = serde_json::from_slice(catalog.json())?;
    let schema = catalog_schema();
    assert!(
        jsonschema::validator_for(&schema)?.is_valid(&value),
        "the full emitted catalog validates against schema v3"
    );
    assert_eq!(
        catalog.json(),
        build_catalog(&archive, options.with_full_provenance())?.json(),
        "same source and selection reproduce exact catalog bytes"
    );
    Ok((catalog, value))
}
