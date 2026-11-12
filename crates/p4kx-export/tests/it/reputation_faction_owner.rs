// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, Kind, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

use super::{build, counts};
use crate::health_origin_assertions as proof;
use crate::reputation_fixture::{
    CONTEXT, FACTION, FACTION_BARE, SCOPE, STANDING_HIGH, STANDING_LOW,
};

use crate::commodity_v3_fixture::{
    self as archive_fixture, DcbOrigin, TestResult, at, guid, text_row,
};

#[path = "reputation_faction_owner_fixture.rs"]
mod fixture;

const MAIN: &str = "/definitions/factionReputations/rows/0";
const PROPERTIES: &str = "/propertiesBb/value";
const DOC: &str = "/exportHealth";

fn export(archive: &Archive, filter: Option<&Filter>) -> TestResult<Value> {
    let mut options = archive_fixture::options();
    options.filter = filter;
    let built = build_catalog(archive, &options).map_err(refusal_summary)?;
    Ok(serde_json::from_slice(built.json())?)
}

fn refusal_summary(error: p4kx_export::CatalogError) -> String {
    let p4kx_export::CatalogError::IncompleteExport(refusal) = error else {
        return error.to_string();
    };
    let reasons = refusal
        .health()
        .diagnostics()
        .iter()
        .filter(|item| item.severity == p4kx_export::ExportDiagnosticSeverity::Error)
        .map(|item| {
            (
                item.kind,
                item.stage,
                item.code,
                item.source_role,
                item.examples()
                    .iter()
                    .map(|example| example.field_text().value())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    format!("normal export rejected by {reasons:?}")
}

#[test]
fn original_faction_reputation_preserves_owner_relations_and_perk_links() -> TestResult {
    let (_folder, archive) = archive_fixture::open(&fixture::owner(false), DcbOrigin::Encrypted)?;
    let catalog = export(&archive, None)?;
    let owner = at(&catalog, MAIN)?;
    assert_static_facts(owner)?;
    assert_relations(&catalog, owner)?;
    assert_schema(&catalog)?;
    check(&catalog, "/exportHealth/status", &json!("complete"))?;
    crate::commodity_v3_source_inventory::reference_closure(&catalog)?;
    assert_filtered_owner(&archive)
}

fn assert_schema(catalog: &Value) -> TestResult {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../schemas/basetool-catalog.schema.json"
    ))?;
    jsonschema::validator_for(&schema)?
        .validate(catalog)
        .map_err(|error| format!("physical FactionReputation rows satisfy schema v3: {error}"))?;
    Ok(())
}

fn check(node: &Value, path: &str, expected: &Value) -> TestResult {
    assert_eq!(at(node, path)?, expected, "original source value at {path}");
    Ok(())
}

fn assert_static_facts(owner: &Value) -> TestResult {
    check(owner, "/background/value", &json!(fixture::BACKGROUND))?;
    check(owner, "/background/containsEncryptedSource", &json!(true))?;
    check(owner, "/guid/value", &json!(guid(fixture::MAIN)))?;
    check(owner, "/geid/value", &json!(fixture::GEID.to_string()))?;
    check(owner, "/geid/containsEncryptedSource", &json!(true))?;
    check(owner, "/hideInDelpihApp/value", &json!(false))?;
    check(owner, "/isNpc/value", &json!(true))?;
    check(owner, "/logo/value", &json!("textures/fixture/logo.dds"))?;
    check(
        owner,
        "/reputationContextPropertiesUi/value/guid/value",
        &json!(guid(fixture::CONTEXT)),
    )?;
    check(
        owner,
        "/allies/value/0/value/guid/value",
        &json!(guid(fixture::ALLY)),
    )?;
    check(
        owner,
        "/enemies/value/0/value/guid/value",
        &json!(guid(fixture::MISSING)),
    )?;
    check(owner, "/enemies/value/0/value/status", &json!("dangling"))?;
    check(
        owner,
        "/enemies/value/1/value/guid/value",
        &json!(guid(fixture::ALLY)),
    )?;
    check(
        owner,
        "/perkReputationRewardList/value/0/value/guid/value",
        &json!(guid(fixture::PERK_LIST)),
    )?;
    Ok(())
}

fn assert_relations(catalog: &Value, owner: &Value) -> TestResult {
    let relation = at(owner, "/hostilityParams/value")?;
    check(
        relation,
        "/scope/value/guid/value",
        &json!(guid(fixture::SCOPE)),
    )?;
    check(
        relation,
        "/standing/value/guid/value",
        &json!(guid(fixture::STANDING)),
    )?;
    let marker = at(relation, "/markerParams/value")?;
    check(
        marker,
        "/icon/value",
        &json!("textures/fixture/relation.dds"),
    )?;
    let text = at(marker, "/description/value/reference")?;
    check(
        text_row(catalog, "gameTexts", text)?,
        "/key/value",
        &json!("@own_description"),
    )?;
    check(
        catalog,
        "/definitions/factions/rows/0/factionReputationRef/value/guid/value",
        &json!(guid(fixture::MAIN)),
    )?;
    Ok(())
}

fn assert_filtered_owner(archive: &Archive) -> TestResult {
    let filter = Filter::new("factionreputation");
    let filtered = export(archive, Some(&filter))?;
    let filtered_owner = at(&filtered, MAIN)?;
    assert_eq!(
        at(
            filtered_owner,
            "/perkReputationRewardList/value/0/value/guid/value"
        )?,
        &json!(guid(fixture::PERK_LIST)),
        "filtering out the shared Record retains the original owner's link",
    );
    let links = at(&filtered, &format!("{DOC}/unresolvedReferences"))?
        .as_array()
        .ok_or("filtered reference observations")?;
    assert!(
        links.iter().any(|link| {
            link["status"] == "excludedByFilter"
                && link["sourceRole"] == "selected"
                && link.pointer("/targetGuid/value") == Some(&json!(guid(fixture::PERK_LIST)))
        }),
        "health distinguishes a filtered perk list from a dangling GUID"
    );
    Ok(())
}

#[test]
fn faction_reputation_blackboard_retains_exact_source_storage_and_legacy_literal() -> TestResult {
    let (_folder, archive) = archive_fixture::open(&fixture::owner(true), DcbOrigin::Plain)?;
    let catalog = export(&archive, None)?;
    let owner = at(&catalog, MAIN)?;
    check(owner, "/description/value/type", &json!("literal"))?;
    check(owner, "/description/value/text", &json!(fixture::LITERAL))?;
    let display_name = at(owner, "/displayName/value")?;
    check(
        text_row(&catalog, "gameTexts", display_name)?,
        "/key/value",
        &json!("@shared"),
    )?;
    assert_blackboard(&catalog, owner)?;
    assert_retained_trigger(&catalog, owner)?;
    check(&catalog, "/exportHealth/status", &json!("complete"))?;
    Ok(())
}

fn assert_blackboard(catalog: &Value, owner: &Value) -> TestResult {
    let entries = at(owner, PROPERTIES)?
        .as_array()
        .ok_or("source BB property slots")?;
    let [lawful, float, int, string, locale, unknown] = entries.as_slice() else {
        return Err("ordered source BB properties".into());
    };
    check(lawful, "/value/name/value", &json!("entityLawful"))?;
    check(
        lawful,
        "/value/dynamicProperty/value/kind",
        &json!("boolean"),
    )?;
    check(
        lawful,
        "/value/dynamicProperty/value/value/value",
        &json!(false),
    )?;
    check(
        lawful,
        "/value/dynamicProperty/value/value/containsEncryptedSource",
        &json!(false),
    )?;
    check(
        float,
        "/value/dynamicProperty/value/value/value/rawBits",
        &json!(format!("{:08x}", fixture::FLOAT_BITS)),
    )?;
    check(int, "/value/dynamicProperty/value/value/value", &json!(-8))?;
    check(
        string,
        "/value/dynamicProperty/value/value/value",
        &json!(fixture::LITERAL),
    )?;
    let localized = at(locale, "/value/dynamicProperty/value/value/value")?;
    check(
        text_row(catalog, "gameTexts", localized)?,
        "/key/value",
        &json!("@shared"),
    )?;
    assert_unknown(unknown)
}

fn assert_unknown(unknown: &Value) -> TestResult {
    check(
        unknown,
        "/value/dynamicProperty/value/sourceType/value",
        &json!(fixture::UNKNOWN),
    )?;
    check(
        unknown,
        "/value/dynamicProperty/value/kind",
        &json!("authored"),
    )?;
    check(
        unknown,
        "/value/dynamicProperty/value/authoredFields/values/0/value/value/value/rawBits",
        &json!(format!("{:08x}", fixture::RAW_BITS)),
    )?;
    Ok(())
}

#[test]
fn contract_dependency_consumes_only_the_original_faction_display_name() -> TestResult {
    let (_folder, archive) = archive_fixture::open(&fixture::owner(false), DcbOrigin::Encrypted)?;
    let filter = Filter::new("missiontype");
    let catalog = export(&archive, Some(&filter))?;
    check(&catalog, "/definitions/factionReputations/rows", &json!([]))?;
    check(&catalog, "/missionTypes/0/typeName/key", &json!("@shared"))?;
    check(&catalog, "/exportHealth/status", &json!("complete"))?;
    Ok(())
}

fn assert_retained_trigger(catalog: &Value, owner: &Value) -> TestResult {
    let guid = at(owner, "/guid/value")?;
    let configuration = proof::rows(catalog, "/configurations/reputationTriggers/rows")?
        .iter()
        .find(|row| row.pointer("/owner/guid/value") == Some(guid))
        .ok_or("physical sandbox trigger owner")?;
    let trigger = at(configuration, "/sandboxTriggers/value/0/value")?;
    check(trigger, "/sourceType/value", &json!(fixture::TRIGGER))?;
    check(trigger, "/kind", &json!("authored"))?;
    let raw = at(trigger, "/authoredFields")?;
    check(raw, "/roots/0/name/value", &json!("fixtureOn"))?;
    check(raw, "/values/0/value/value/value", &json!(true))
}

fn faction_reputation(catalog: &Value, record: u128) -> TestResult<&Value> {
    let guid = guid(record);
    proof::rows(catalog, "/definitions/factionReputations/rows")?
        .iter()
        .find(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(guid.as_str()))
        .ok_or_else(|| "source-backed faction reputation".into())
}

#[test]
fn a_faction_lists_its_links_and_reads_lawful_from_the_property() -> TestResult {
    let (built, catalog) = build(None)?;
    assert_eq!(catalog.get("factionReputations"), None);
    let owner = faction_reputation(&catalog, FACTION)?;
    proof::present(
        owner,
        "className",
        &json!("FactionReputation_Acme"),
        Some(true),
    )?;
    proof::present(owner, "isNpc", &json!(false), Some(true))?;
    assert_eq!(
        proof::at(owner, "/reputationContextPropertiesUi/value/guid/value")?,
        &json!(guid(CONTEXT)),
    );
    assert_eq!(
        proof::at(owner, "/allies/value/0/value/guid/value")?,
        &json!(guid(FACTION_BARE)),
    );
    assert_eq!(proof::at(owner, "/enemies/value")?, &json!([]));
    for (relation, standing) in [
        ("hostilityParams", STANDING_LOW),
        ("alliedParams", STANDING_HIGH),
    ] {
        assert_eq!(
            proof::at(owner, &format!("/{relation}/value/scope/value/guid/value"))?,
            &json!(guid(SCOPE)),
        );
        assert_eq!(
            proof::at(
                owner,
                &format!("/{relation}/value/standing/value/guid/value")
            )?,
            &json!(guid(standing)),
        );
    }
    check_faction_name_and_lawful(&catalog, owner)?;
    assert_eq!(
        counts(&built, Kind::FactionReputations),
        Some((2, 1)),
        "the test faction is a leftover"
    );
    Ok(())
}

fn check_faction_name_and_lawful(catalog: &Value, owner: &Value) -> TestResult {
    let lawful = proof::rows(owner, "/propertiesBb/value")?
        .iter()
        .find(|slot| {
            slot.pointer("/value/name/value").and_then(Value::as_str) == Some("entityLawful")
        })
        .ok_or("original entityLawful property")?;
    assert_eq!(
        proof::at(lawful, "/value/dynamicProperty/value/kind")?,
        "boolean",
        "the original property uses Bool storage"
    );
    assert_eq!(
        proof::at(lawful, "/value/dynamicProperty/value/value/value")?,
        true,
        "the original lawful property value"
    );
    let reference = proof::at(owner, "/displayName/value")?;
    let display = text_row(catalog, "gameTexts", reference)?;
    assert_eq!(
        proof::at(display, "/key/value")?,
        "@faction_acme",
        "the original localization key"
    );
    assert_eq!(
        proof::at(display, "/translations/value/en/displayText/value")?,
        "Acme Emporium",
        "the original English display text"
    );
    Ok(())
}

#[test]
fn a_faction_preserves_default_class_slots_and_empty_collections() -> TestResult {
    let (_, catalog) = build(None)?;
    let owner = faction_reputation(&catalog, FACTION_BARE)?;
    proof::cell(owner, "reputationContextPropertiesUi", "null")?;
    for relation in ["hostilityParams", "alliedParams"] {
        let body = proof::at(owner, &format!("/{relation}/value"))?;
        proof::cell(body, "scope", "null")?;
        proof::cell(body, "standing", "null")?;
        proof::cell(body, "markerParams", "absent")?;
    }
    assert_eq!(proof::at(owner, "/isNpc/value")?, false);
    for array in ["allies", "enemies", "propertiesBb"] {
        assert_eq!(proof::at(owner, &format!("/{array}/value"))?, &json!([]));
    }
    Ok(())
}
