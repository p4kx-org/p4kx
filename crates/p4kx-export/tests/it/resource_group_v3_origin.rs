// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::CatalogPublicationMode;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at};
use crate::commodity_v3_source_inventory::{body, header};
use crate::commodity_v3_source_inventory::{reference_closure, uses};
use crate::resource_group_v3_fixture as fixture;

#[test]
fn group_headers_arrays_and_reference_headers_retain_exact_independent_source_cells() -> TestResult
{
    let (_, catalog) = fixture::diagnostic(&fixture::graph())?;
    let group = fixture::row(&catalog, "definitions/resourceGroups", 0)?;
    for (field, input, role) in [
        ("guid", header("guid"), "payload"),
        ("recordName", header("recordName"), "payload"),
        ("recordIndex", header("recordIndex"), "selector"),
        ("sourceType", header("sourceType"), "selector"),
        ("recordPath", header("fileName"), "conversion"),
        (
            "defaultThumbnailPath",
            body("defaultThumbnailPath"),
            "payload",
        ),
    ] {
        assert_eq!(
            uses(&catalog, at(group, &format!("/{field}"))?)?,
            BTreeSet::from([(0, input, role.to_owned())]),
            "the canonical ResourceGroup source contract is preserved"
        );
    }
    assert_member_origins(&catalog)?;
    reference_closure(&catalog)?;
    Ok(())
}

#[test]
fn parent_choice_uses_only_actual_prior_slots_and_not_later_faults() -> TestResult {
    for before in [false, true] {
        check_parent_choice(before)?;
    }
    Ok(())
}

#[test]
fn direct_and_derived_group_facts_keep_original_encrypted_and_unknown_dcb_origins() -> TestResult {
    for (origin, expected) in [
        (DcbOrigin::Encrypted, json!(true)),
        (DcbOrigin::Unknown, Value::Null),
    ] {
        let (_, catalog) = fixture::build(
            &fixture::clean(),
            origin,
            CatalogPublicationMode::DiagnosticPartial,
            None,
        )?;
        for dataset in [
            "definitions/resourceGroups",
            "declarations/resourceGroupMembers",
            "projections/resourceGroupParents",
        ] {
            for row in fixture::rows(&catalog, dataset)? {
                assert_origins(row, &expected)?;
            }
        }
        reference_closure(&catalog)?;
    }
    Ok(())
}

fn assert_origins(value: &Value, expected: &Value) -> TestResult {
    if value.get("sourceUsesRefs").is_some() && value.get("state").is_some() {
        assert_eq!(
            at(value, "/containsEncryptedSource")?,
            expected,
            "each sourced group datum exposes its direct marker"
        );
    }
    match value {
        Value::Array(array) => {
            for child in array {
                assert_origins(child, expected)?;
            }
        }
        Value::Object(object) => {
            for child in object.values() {
                assert_origins(child, expected)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[test]
fn group_text_domains_and_languages_keep_their_own_sources_and_protection() -> TestResult {
    let (_, catalog) = fixture::build(
        &fixture::clean(),
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        None,
    )?;
    let row = fixture::row(&catalog, "definitions/resourceGroups", 0)?;
    let name = common::text_row(&catalog, "publicNames", at(row, "/name/value")?)?;
    let description = common::text_row(&catalog, "gameTexts", at(row, "/description/value")?)?;
    assert_ne!(
        at(name, "/id/value")?,
        at(description, "/id/value")?,
        "the canonical ResourceGroup source contract is preserved"
    );
    for text in [name, description] {
        assert_eq!(
            at(text, "/key/value")?,
            &json!("@shared"),
            "the canonical ResourceGroup source contract is preserved"
        );
        assert_text_form_markers(text)?;
    }
    assert_eq!(
        at(row, "/name/containsEncryptedSource")?,
        &json!(false),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(row, "/description/containsEncryptedSource")?,
        &json!(false),
        "the canonical ResourceGroup source contract is preserved"
    );
    Ok(())
}

fn assert_text_form_markers(text: &Value) -> TestResult {
    for (language, expected) in [("en", true), ("de", false)] {
        let forms = crate::text_forms_assertions::forms(text, language)?;
        for field in ["sourceText", "displayText"] {
            crate::text_forms_assertions::direct(&forms[field])?;
            assert_eq!(
                at(&forms[field], "/containsEncryptedSource")?,
                &json!(expected),
                "each group text form retains its actual language-input origin"
            );
        }
    }
    Ok(())
}

fn assert_member_origins(catalog: &Value) -> TestResult {
    let members = fixture::row(catalog, "declarations/resourceGroupMembers", 0)?;
    assert_eq!(
        uses(catalog, at(members, "/groups")?)?,
        BTreeSet::from([(0, body("groups"), "payload".to_owned())]),
        "the canonical ResourceGroup source contract is preserved"
    );
    let edge = at(members, "/groups/value/3")?;
    assert_eq!(
        uses(catalog, edge)?,
        BTreeSet::from([(0, body("groups[3]"), "payload".to_owned())]),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        uses(catalog, at(edge, "/value/guid")?)?,
        uses(catalog, edge)?,
        "the canonical ResourceGroup source contract is preserved"
    );
    let target = at(edge, "/value/targetRecordIndex/value")?
        .as_u64()
        .ok_or("physical target")?;
    for (field, source, role) in [
        ("targetRecordIndex", "recordIndex", "selector"),
        ("targetType", "sourceType", "selector"),
        ("recordPath", "fileName", "conversion"),
    ] {
        assert_eq!(
            uses(catalog, at(edge, &format!("/value/{field}"))?)?,
            BTreeSet::from([(target, header(source), role.to_owned())]),
            "the canonical ResourceGroup source contract is preserved"
        );
    }
    Ok(())
}

fn check_parent_choice(before: bool) -> TestResult {
    let (_, catalog) = fixture::diagnostic(&fixture::fault(before))?;
    let parent = fixture::row(&catalog, "projections/resourceGroupParents", 2)?;
    let decision = at(parent, "/parent")?;
    let winner = u64::from(before);
    assert_eq!(
        at(decision, "/value/targetRecordIndex/value")?,
        &json!(winner),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(decision, "/sourceSetComplete")?,
        &json!(!before),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_parent_prefix(before, &catalog, decision, winner)?;
    Ok(())
}

fn assert_parent_prefix(
    before: bool,
    catalog: &Value,
    decision: &Value,
    winner: u64,
) -> TestResult {
    let actual = uses(catalog, decision)?;
    assert!(
        actual.contains(&(winner, body("groups[0]"), "selector".to_owned())),
        "the original chosen edge contributes"
    );
    assert!(
        actual.contains(&(winner, header("guid"), "payload".to_owned())),
        "the chosen parent identity contributes"
    );
    assert!(
        actual.contains(&(2, header("guid"), "selector".to_owned())),
        "the child identity selects its parent"
    );
    if before {
        assert!(
            !actual.contains(&(0, body("groups"), "attempted".to_owned())),
            "attempts are observations rather than retained contributors"
        );
        let inventory = at(catalog, "/provenance/uses")?
            .as_array()
            .ok_or("source observations")?;
        assert!(
            inventory
                .iter()
                .any(
                    |used| used.get("role").and_then(Value::as_str) == Some("attempted")
                        && used
                            .pointer("/location/recordIndex")
                            .and_then(Value::as_u64)
                            == Some(0)
                        && used.pointer("/location/field").and_then(Value::as_str)
                            == Some("groups")
                ),
            "the earlier failed read remains an actual observation"
        );
        assert!(
            at(decision, "/containsEncryptedSource")?.is_null(),
            "an incomplete plain contribution set stays unknown"
        );
    } else {
        assert!(
            !actual.iter().any(|(record, _, _)| *record == 1),
            "the later failed group is not a counterfactual contributor"
        );
        assert_eq!(
            at(decision, "/containsEncryptedSource")?,
            &json!(false),
            "the canonical ResourceGroup source contract is preserved"
        );
    }
    Ok(())
}

#[test]
fn an_unreadable_first_header_retains_the_known_physical_order_selector() -> TestResult {
    let (built, _) = crate::catalog_crafting_support::build_from_data(
        fixture::unreadable_first_path()?,
        None,
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = fixture::row(&catalog, "projections/resourceGroupParents", 2)?;
    let choice = at(row, "/parent")?;
    assert_eq!(
        at(choice, "/value/targetRecordIndex/value")?,
        &json!(1),
        "the established later readable winner remains explicit"
    );
    assert_eq!(
        at(choice, "/sourceSetComplete")?,
        &json!(false),
        "the earlier metadata failure can hide a different winner"
    );
    let used = uses(&catalog, choice)?;
    assert!(
        used.contains(&(0, header("recordIndex"), "selector".to_owned())),
        "the known original occurrence remains an actual order selector"
    );
    assert!(
        !used
            .iter()
            .any(|(index, field, _)| *index == 0 && field == &header("fileName")),
        "failed metadata is not a retained contribution"
    );
    let inventory = at(&catalog, "/provenance/uses")?
        .as_array()
        .ok_or("source observations")?;
    assert!(
        inventory.iter().any(
            |used| used.get("role").and_then(Value::as_str) == Some("attempted")
                && used.pointer("/location/type").and_then(Value::as_str) == Some("subresource")
                && used.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeRecordHeader")
                && used.pointer("/location/indices") == Some(&json!(["0"]))
                && used.pointer("/location/field").and_then(Value::as_str) == Some("fileName")
        ),
        "the actual failed header read remains observable separately"
    );
    Ok(())
}

#[test]
fn a_no_parent_result_excludes_identity_inputs_used_only_for_other_children() -> TestResult {
    let (_, catalog) = fixture::diagnostic(&fixture::graph())?;
    let row = fixture::row(&catalog, "projections/resourceGroupParents", 4)?;
    let parent = at(row, "/parent")?;
    assert!(
        at(parent, "/value")?.is_null(),
        "the fixture has no selected parent for this GUID"
    );
    let actual = uses(&catalog, parent)?;
    assert!(
        actual.contains(&(4, header("guid"), "selector".to_owned())),
        "the queried child GUID contributes to its own lookup"
    );
    for candidate in [0, 1, 5, 6] {
        assert!(
            !actual
                .iter()
                .any(|(index, field, _)| *index == candidate && field == &header("guid")),
            "a different child's self-check does not contribute to this no-parent result"
        );
    }
    Ok(())
}
