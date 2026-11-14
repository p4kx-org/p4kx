// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, build_catalog};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at, guid};

#[path = "location_vocabulary_fixture.rs"]
mod fixture;

const CLASS_ROWS: &str = "/definitions/locationClasses/rows";
const CLASS_REGISTRY: &str = "/definitions/locationRegistries/classes/rows/0";

#[test]
fn location_definitions_keep_identity_without_payload_fields() -> TestResult {
    let catalog = export(DcbOrigin::Encrypted, None)?;
    let classes = array(&catalog, CLASS_ROWS)?;
    let [first, second] = classes.as_slice() else {
        return Err("two distinct physical classes".into());
    };
    assert_eq!(
        at(first, "/guid/value")?,
        &json!(guid(fixture::CLASS_A)),
        "GUID ordering is stable"
    );
    assert_eq!(
        at(second, "/guid/value")?,
        &json!(guid(fixture::CLASS_B)),
        "the second Record keeps its GUID"
    );
    let scopes = array(first, "/scopesVisibleTo/value")?;
    assert_eq!(
        scopes
            .iter()
            .map(|scope| scope["value"].as_str())
            .collect::<Vec<_>>(),
        [
            Some(fixture::LOCAL),
            Some(fixture::LOCAL),
            Some(fixture::FUTURE)
        ],
        "the open source vocabulary and duplicate positions survive"
    );
    assert!(
        scopes
            .iter()
            .all(|scope| scope["containsEncryptedSource"] == true),
        "each scope label has an immediate marker"
    );
    for (path, original) in [
        ("/definitions/locationFunctions/rows/0", fixture::FUNCTION),
        ("/definitions/locationSizes/rows/0", fixture::SIZE),
    ] {
        let row = at(&catalog, path)?;
        assert_eq!(
            at(row, "/guid/value")?,
            &json!(guid(original)),
            "identity is enough to admit an empty definition"
        );
        assert!(
            array(row, "/authoredFields/roots")?.is_empty(),
            "no authored payload fields"
        );
    }
    verify_document(&catalog)
}

#[test]
fn location_registries_keep_original_slots_and_filtered_targets() -> TestResult {
    let filter = Filter::new("*/fixture_location/registries/*");
    let catalog = export(DcbOrigin::Plain, Some(&filter))?;
    assert!(
        array(&catalog, CLASS_ROWS)?.is_empty(),
        "the physical target rows are filtered"
    );
    assert_eq!(
        at(&catalog, "/definitions/locationClasses/availability")?,
        "available",
        "filtering does not erase a source declaration"
    );
    let class_registry = at(&catalog, CLASS_REGISTRY)?;
    assert_eq!(
        at(class_registry, "/guid/value")?,
        &json!(guid(fixture::CLASSES)),
        "the class registry has independent identity"
    );
    let members = array(class_registry, "/members/value")?;
    let [second, missing, first, repeated, null] = members.as_slice() else {
        return Err("five original registry slots".into());
    };
    for (member, original) in [
        (second, fixture::CLASS_B),
        (missing, fixture::MISSING),
        (first, fixture::CLASS_A),
        (repeated, fixture::CLASS_A),
    ] {
        assert_eq!(
            at(member, "/value/guid/value")?,
            &json!(guid(original)),
            "authored registry order"
        );
        assert_eq!(
            at(member, "/containsEncryptedSource")?,
            false,
            "original selector from the clear DCB"
        );
    }
    assert_eq!(
        at(missing, "/value/status")?,
        "dangling",
        "the original missing GUID remains visible"
    );
    assert_eq!(at(null, "/state")?, "null", "explicit null slot");
    let function_registry = at(&catalog, "/definitions/locationRegistries/functions/rows/0")?;
    assert_eq!(
        at(function_registry, "/guid/value")?,
        &json!(guid(fixture::FUNCTIONS)),
        "the registry has independent identity"
    );
    assert!(
        array(function_registry, "/members/value")?.is_empty(),
        "present empty is distinct from an absent member array"
    );
    let size_registry = at(&catalog, "/definitions/locationRegistries/sizes/rows/0")?;
    assert_eq!(
        at(size_registry, "/guid/value")?,
        &json!(guid(fixture::SIZES)),
        "the size registry is distinct"
    );
    assert_eq!(
        at(size_registry, "/members/value/0/value/guid/value")?,
        &json!(guid(fixture::SIZE)),
        "actual size member"
    );
    let diagnostics = array(&catalog, "/exportHealth/unresolvedReferences")?;
    let first_guid = json!(guid(fixture::CLASS_A));
    assert!(
        diagnostics
            .iter()
            .any(|entry| entry["status"] == "excludedByFilter"
                && entry.pointer("/targetGuid/value") == Some(&first_guid)),
        "filtered dictionary target is explicit in health"
    );
    Ok(())
}

fn export(origin: DcbOrigin, filter: Option<&Filter>) -> TestResult<Value> {
    let (_directory, archive) = common::open(&fixture::database(), origin)?;
    let mut options = common::options();
    options.filter = filter;
    let built = build_catalog(&archive, &options).map_err(|error| error.to_string())?;
    Ok(serde_json::from_slice(built.json())?)
}

fn array<'a>(root: &'a Value, path: &str) -> TestResult<&'a Vec<Value>> {
    at(root, path)?
        .as_array()
        .ok_or_else(|| format!("array at {path}").into())
}

#[test]
fn mission_location_templates_link_to_the_physical_classification() -> TestResult {
    let (_directory, archive) = common::open(
        &fixture::template_database("MissionLocationData"),
        DcbOrigin::Encrypted,
    )?;
    let built = build_catalog(&archive, &common::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = at(&catalog, "/missionLocationTemplates/0")?;
    let classification = at(row, "/classification")?;
    assert_eq!(at(classification, "/state")?, "present");
    assert_eq!(at(classification, "/containsEncryptedSource")?, true);
    for (field, table, target) in [
        ("locationClass", "locationClasses", fixture::CLASS_A),
        ("locationFunction", "locationFunctions", fixture::FUNCTION),
        ("locationSize", "locationSizes", fixture::SIZE),
    ] {
        let link = at(classification, &format!("/value/{field}"))?;
        assert_eq!(at(link, "/containsEncryptedSource")?, true, "{field}");
        assert_eq!(
            at(link, "/value/guid/value")?,
            &json!(guid(target)),
            "{field}"
        );
        let index = at(link, "/value/targetRecordIndex/value")?;
        let exported = array(&catalog, &format!("/definitions/{table}/rows"))?;
        assert!(
            exported
                .iter()
                .any(|row| row.pointer("/recordIndex/value") == Some(index)),
            "{field} joins the exact target Record"
        );
    }
    let sparse = at(&catalog, "/missionLocationTemplates/1/classification/value")?;
    assert_eq!(at(sparse, "/locationClass/state")?, "null");
    assert_eq!(at(sparse, "/locationSize/value/status")?, "dangling");
    assert_eq!(
        at(sparse, "/locationSize/value/guid/value")?,
        &json!(guid(fixture::MISSING))
    );
    verify_document(&catalog)
}

#[test]
fn mission_location_templates_report_the_rejected_class_declaration() -> TestResult {
    let (_directory, archive) = common::open(
        &fixture::template_database("OtherLocationData"),
        DcbOrigin::Plain,
    )?;
    let mut options = common::options();
    options.publication_mode = p4kx_export::CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = at(&catalog, "/missionLocationTemplates/0")?;
    assert_eq!(at(row, "/classification/state")?, "unsupported");
    assert_eq!(at(row, "/classification/containsEncryptedSource")?, false);
    assert_eq!(
        at(row, "/disabled")?,
        true,
        "the existing flag still uses its source"
    );
    let diagnostics = array(&catalog, "/exportHealth/diagnostics")?;
    assert!(
        diagnostics
            .iter()
            .any(|row| row["kind"] == "missionLocationTemplates"
                && row["stage"] == "projection"
                && row["code"] == "unsupportedValue"),
        "typed projection diagnosis belongs to the template"
    );
    Ok(())
}

fn verify_document(catalog: &Value) -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    if let Err(error) = validator.validate(catalog) {
        return Err(format!("{}: {}", error.instance_path(), error).into());
    }
    crate::commodity_v3_source_inventory::reference_closure(catalog)
}
