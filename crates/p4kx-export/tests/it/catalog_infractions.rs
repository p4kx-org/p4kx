// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter, build_catalog};
use serde_json::{Value, json};

use self::fixture::{DEFINITION, ENTITY, FIRST, FUTURE, MISSING, PARENT, PRISON, SET};
use crate::commodity_v3_fixture::{self as archive, DcbOrigin, TestResult, at, guid, text_row};
use crate::health_origin_assertions as proof;

#[path = "infraction_fixture.rs"]
mod fixture;

#[test]
fn catalog_infractions_keep_authored_defaults_text_and_unknown_conditions() -> TestResult {
    let (_folder, input) = archive::open(&fixture::correct(), DcbOrigin::Encrypted)?;
    let catalog = document(&input, None, false)?;
    let infraction = at(&catalog, "/definitions/infractions/rows/0")?;
    proof::present(infraction, "guid", &json!(guid(DEFINITION)), Some(true))?;
    assert_text(&catalog, at(infraction, "/name")?, "publicNames")?;
    assert_text(&catalog, at(infraction, "/description")?, "gameTexts")?;
    let triggers = proof::rows(infraction, "/triggers/value")?;
    assert_eq!(triggers.len(), 3, "every authored trigger occurrence");
    for (row, label) in triggers.iter().zip([FUTURE, FIRST, FUTURE]) {
        assert_sourced(row, &json!(label), true)?;
    }
    assert_ne!(
        at(&triggers[0], "/sourceUsesRefs")?,
        at(&triggers[2], "/sourceUsesRefs")?,
        "repeated enum text has distinct source occurrences"
    );
    let defaults = at(infraction, "/defaultParameters/value")?;
    proof::present(defaults, "isFelony", &json!(-1), Some(true))?;
    proof::present(defaults, "graceAllowance", &json!(0), Some(true))?;
    proof::present(
        defaults,
        "ignoreIfAgainstPartyMember",
        &json!(-1),
        Some(true),
    )?;
    proof::present(
        defaults,
        "gracePeriod",
        &json!({"rawBits":"80000000","finite":-0.0}),
        Some(true),
    )?;
    assert_text(
        &catalog,
        at(defaults, "/graceWarnings/value/0")?,
        "gameTexts",
    )?;
    verify_future_conditions(defaults)?;
    proof::closure(&catalog)
}

#[test]
fn catalog_infractions_keep_shared_sets_conflicting_overrides_and_cyclic_parents() -> TestResult {
    let (_folder, input) = archive::open(&fixture::correct(), DcbOrigin::Plain)?;
    let catalog = document(&input, None, false)?;
    let set = at(&catalog, "/definitions/infractionSets/rows/0")?;
    proof::present(set, "guid", &json!(guid(SET)), Some(false))?;
    let uses = proof::rows(set, "/infractions/value")?;
    assert_eq!(
        uses.len(),
        4,
        "the authored array has all repeated occurrences"
    );
    for (index, (target, flag)) in [(DEFINITION, 0), (MISSING, 2), (DEFINITION, 3), (ENTITY, 4)]
        .into_iter()
        .enumerate()
    {
        let use_row = at(&uses[index], "/value")?;
        assert_eq!(
            at(use_row, "/definition/value/guid/value")?,
            &json!(guid(target)),
            "the exact original reference identity in source order"
        );
        let overrides = at(use_row, "/parameterOverrides/value")?;
        proof::present(overrides, "isFelony", &json!(flag), Some(false))?;
        proof::present(
            overrides,
            "gracePeriod",
            &json!({"rawBits":"7fc01072","finite":null}),
            Some(false),
        )?;
    }
    assert_eq!(
        at(&uses[1], "/value/definition/value/status")?,
        &json!("dangling"),
        "dangling GUID"
    );
    assert_eq!(
        at(&uses[3], "/value/definition/value/status")?,
        &json!("unexpectedTargetType"),
        "incompatible physical reference retains its GUID"
    );
    let prison = at(&catalog, "/definitions/jurisdictions/rows/0")?;
    let parent = at(&catalog, "/definitions/jurisdictions/rows/1")?;
    verify_jurisdiction(prison)?;
    assert_eq!(
        at(parent, "/parentJurisdiction/value/guid/value")?,
        &json!(guid(PRISON)),
        "cyclic parent retains original GUID"
    );
    proof::closure(&catalog)
}

#[test]
fn catalog_infractions_preserve_references_to_filtered_records() -> TestResult {
    let (_folder, input) = archive::open(&fixture::correct(), DcbOrigin::Plain)?;
    let filter = Filter::new("selected/*");
    let catalog = document(&input, Some(&filter), false)?;
    let rows = proof::rows(&catalog, "/definitions/jurisdictions/rows")?;
    assert_eq!(rows.len(), 1, "filter selects the prison only");
    assert!(
        proof::rows(&catalog, "/definitions/infractionSets/rows")?.is_empty(),
        "set stays filtered"
    );
    assert!(
        proof::rows(&catalog, "/definitions/infractions/rows")?.is_empty(),
        "infraction stays filtered"
    );
    verify_jurisdiction(&rows[0])?;
    assert_eq!(
        at(&catalog, "/completeness/jurisdictions/status")?,
        "partial",
        "the filter is distinct from table availability"
    );
    let links = proof::rows(&catalog, "/exportHealth/unresolvedReferences")?;
    for target in [DEFINITION, SET, PARENT] {
        let expected = json!(guid(target));
        assert!(
            links.iter().any(|link| {
                link.pointer("/targetGuid/value") == Some(&expected)
                    && link["kind"] == "jurisdictions"
                    && link["status"] == "excludedByFilter"
            }),
            "the original target has an explicit filter result"
        );
    }
    proof::closure(&catalog)
}

#[test]
fn catalog_infractions_reject_a_different_nominal_trigger_enum() -> TestResult {
    let (_folder, input) = archive::open(&fixture::wrong_trigger_declaration(), DcbOrigin::Plain)?;
    let catalog = document(&input, None, true)?;
    let infraction = at(&catalog, "/definitions/infractions/rows/0")?;
    proof::cell(infraction, "triggers", "unsupported")?;
    assert!(
        serde_json::to_string(at(infraction, "/authoredFields")?)?.contains(FUTURE),
        "unprojected original trigger labels remain available in the source arena"
    );
    proof::closure(&catalog)
}

#[test]
fn catalog_infractions_distinguish_unreadable_absent_null_and_empty() -> TestResult {
    let (_folder, input) = archive::open(&fixture::sparse(), DcbOrigin::Encrypted)?;
    let catalog = document(&input, None, true)?;
    let infraction = at(&catalog, "/definitions/infractions/rows/0")?;
    proof::cell(infraction, "description", "unreadable")?;
    proof::cell(infraction, "triggers", "absent")?;
    let defaults = at(infraction, "/defaultParameters/value")?;
    proof::cell(defaults, "isFelony", "absent")?;
    let warnings = proof::rows(defaults, "/graceWarnings/value")?;
    assert_eq!(warnings.len(), 2, "every original localized slot survives");
    assert_text(&catalog, &warnings[0], "gameTexts")?;
    assert_eq!(
        at(&warnings[1], "/state")?,
        "unreadable",
        "the malformed original locale slot is distinct from a missing slot"
    );
    let jurisdiction = at(&catalog, "/definitions/jurisdictions/rows/0")?;
    proof::cell(jurisdiction, "parentJurisdiction", "null")?;
    proof::present(jurisdiction, "prohibitedGoods", &json!([]), Some(true))?;
    proof::cell(jurisdiction, "prohibitedResources", "absent")?;
    proof::closure(&catalog)
}

#[test]
fn catalog_infractions_schema_requires_owned_cells_and_protects_warnings() -> TestResult {
    let (_folder, input) = archive::open(&fixture::correct(), DcbOrigin::Encrypted)?;
    let catalog = document(&input, None, false)?;
    let schema = p4kx_export::catalog_schema();
    assert_eq!(
        at(
            &schema,
            "/$defs/InfractionParameters/properties/graceWarnings/x-gameText"
        )?,
        true,
        "the protected text annotation applies to the authored warning array"
    );
    let validator = jsonschema::validator_for(&schema)?;
    validator
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    let mut omitted = catalog;
    omitted
        .pointer_mut("/definitions/infractions/rows/0/defaultParameters/value")
        .and_then(Value::as_object_mut)
        .and_then(|parameters| parameters.remove("isFelony"))
        .ok_or("typed default flag cell")?;
    assert!(
        !validator.is_valid(&omitted),
        "a required authored field cannot disappear"
    );
    Ok(())
}

fn verify_future_conditions(defaults: &Value) -> TestResult<()> {
    let [root] = proof::rows(defaults, "/authoredFields/roots")? else {
        return Err("expected exactly one unprojected authored field".into());
    };
    assert_eq!(
        at(root, "/name/value")?,
        "futureConditions",
        "exact original condition root"
    );
    let nodes = proof::rows(defaults, "/authoredFields/values")?;
    let condition = original_node(nodes, at(root, "/value")?)?;
    let negation = at(condition, "/fields/0")?;
    assert_eq!(
        at(negation, "/name/value")?,
        "negated",
        "the original member name"
    );
    let array = original_node(nodes, at(negation, "/value")?)?;
    let body = original_node(nodes, at(array, "/elements/0")?)?;
    let allow = at(body, "/fields/0")?;
    assert_eq!(
        at(allow, "/name/value")?,
        "allow",
        "the original nested flag name"
    );
    assert_eq!(
        at(original_node(nodes, at(allow, "/value")?)?, "/value")?,
        false,
        "the actual negation flag value"
    );
    let scale = original_node(nodes, at(body, "/fields/1/value")?)?;
    assert_eq!(
        at(scale, "/value/rawBits")?,
        "7fc01073",
        "exact nested Single bits"
    );
    Ok(())
}

fn original_node<'rows>(rows: &'rows [Value], reference: &Value) -> TestResult<&'rows Value> {
    let index: usize = at(reference, "/valueIndex")?
        .as_str()
        .ok_or("original value index")?
        .parse()?;
    at(
        rows.get(index).ok_or("referenced source arena node")?,
        "/value/value",
    )
}

fn verify_jurisdiction(prison: &Value) -> TestResult<()> {
    proof::present(prison, "guid", &json!(guid(PRISON)), Some(false))?;
    proof::present(prison, "isPrison", &json!(true), Some(false))?;
    proof::present(prison, "baseFine", &json!(0), Some(false))?;
    proof::present(
        prison,
        "respectsParentJurisdictionLaws",
        &json!(false),
        Some(false),
    )?;
    assert_eq!(
        at(prison, "/parentJurisdiction/value/guid/value")?,
        &json!(guid(PARENT)),
        "the actual parent target"
    );
    assert_eq!(
        at(prison, "/infractionSets/value/0/value/guid/value")?,
        &json!(guid(SET)),
        "shared set remains a physical link"
    );
    let impound = at(prison, "/impoundingDefinitions/value/0/value")?;
    proof::present(impound, "impoundingFineUec", &json!(0), Some(false))?;
    proof::present(impound, "trigger", &json!(FUTURE), Some(false))?;
    let substance = at(prison, "/controlledSubstanceClasses/value/0/value")?;
    proof::present(
        substance,
        "maxPossessionScu",
        &json!({"rawBits":"00000000","finite":0.0}),
        Some(false),
    )?;
    Ok(())
}

fn assert_text(catalog: &Value, cell: &Value, domain: &str) -> TestResult<()> {
    let text = text_row(catalog, domain, at(cell, "/value")?)?;
    assert_eq!(
        at(text, "/key/value")?,
        &json!("@shared"),
        "exact original locale key and domain"
    );
    assert_eq!(
        at(cell, "/containsEncryptedSource")?,
        &json!(true),
        "independent original text receipt"
    );
    Ok(())
}

fn assert_sourced(row: &Value, expected: &Value, encrypted: bool) -> TestResult<()> {
    assert_eq!(at(row, "/value")?, expected, "the exact ordered enum label");
    assert_eq!(
        at(row, "/containsEncryptedSource")?,
        &json!(encrypted),
        "the exact enum source origin"
    );
    assert_eq!(
        at(row, "/sourceSetComplete")?,
        &json!(true),
        "the source set of this occurrence"
    );
    Ok(())
}

fn document(
    input: &p4kx_extraction::Archive,
    filter: Option<&Filter>,
    partial: bool,
) -> TestResult<Value> {
    let mut options = archive::options();
    options.filter = filter;
    if partial {
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    }
    Ok(serde_json::from_slice(
        build_catalog(input, &options)?.json(),
    )?)
}
