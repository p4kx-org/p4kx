// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Resource relationships with exact physical provenance and publication health.

use std::error::Error;

use p4kx_export::{CatalogError, Filter};
use serde_json::{Value, json};

use crate::catalog_crafting_support;
use crate::catalog_crafting_support::commodity::{self, TextDomain};
use crate::catalog_crafting_support::groups;
use crate::health_origin_assertions as health;
use crate::resource_edges_fixture as fixture;

type TestResult = Result<(), Box<dyn Error>>;

fn row<'catalog>(
    catalog: &'catalog Value,
    kind: &str,
    guid: u128,
) -> Result<&'catalog Value, Box<dyn Error>> {
    let wanted = catalog_crafting_support::text(guid);
    catalog[kind]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["guid"] == wanted))
        .ok_or_else(|| format!("the synthetic {kind} row").into())
}

#[test]
fn a_dangling_first_resource_is_not_promoted_to_a_later_valid_resource() -> TestResult {
    let (_, catalog) = catalog_crafting_support::build_from(&fixture::dangling_first(), None)?;
    let commodity = commodity::presentation(
        &catalog,
        &catalog_crafting_support::text(fixture::COMMODITY),
    )?;
    assert_eq!(
        commodity["subtypeGuid"]["value"],
        catalog_crafting_support::text(fixture::MISSING)
    );
    assert_eq!(catalog["exportHealth"]["status"], "complete");
    Ok(())
}

#[test]
fn a_null_composition_link_still_allows_the_first_non_null_resource() -> TestResult {
    let (_, catalog) = catalog_crafting_support::build_from(&fixture::null_first(), None)?;
    assert_eq!(
        commodity::presentation(
            &catalog,
            &catalog_crafting_support::text(fixture::COMMODITY)
        )?["subtypeGuid"]["value"],
        catalog_crafting_support::text(fixture::RESOURCE)
    );
    Ok(())
}

#[test]
fn a_weak_record_link_keeps_its_actual_second_alias_target() -> TestResult {
    let filter = Filter::new("included");
    let (_, catalog) = catalog_crafting_support::build_from(
        &fixture::aliases(fixture::LinkStorage::WeakRecordPointer),
        Some(&filter),
    )?;
    commodity::assert_text(
        &catalog,
        &commodity::presentation(
            &catalog,
            &catalog_crafting_support::text(fixture::COMMODITY),
        )?["name"],
        TextDomain::PublicName,
        Some("@fixture_emitted"),
        None,
        None,
    )?;
    assert_eq!(
        row(&catalog, "resourceTypes", fixture::RESOURCE)?["name"]["key"],
        "@fixture_emitted"
    );
    assert_eq!(catalog["exportHealth"]["status"], "complete");
    Ok(())
}

#[test]
fn another_emitted_guid_alias_cannot_stand_in_for_the_actual_filtered_target() -> TestResult {
    let filter = Filter::new("included");
    let database = fixture::aliases(fixture::LinkStorage::TypedReference);
    let Err(error) = catalog_crafting_support::build_from(&database, Some(&filter)) else {
        return Err("a different physical GUID alias cannot satisfy the source link".into());
    };
    assert!(matches!(
        error.downcast_ref::<CatalogError>(),
        Some(CatalogError::IncompleteExport(_))
    ));
    let (_, catalog) = catalog_crafting_support::diagnostic_from(&database, Some(&filter))?;
    let references = catalog["exportHealth"]["unresolvedReferences"]
        .as_array()
        .ok_or("the unresolved references")?;
    assert!(
        references
            .iter()
            .any(|reference| reference["status"] == "ambiguousIdentity"
                && reference["targetRecordIndex"]["value"] == 0)
    );
    commodity::assert_text(
        &catalog,
        &commodity::presentation(
            &catalog,
            &catalog_crafting_support::text(fixture::COMMODITY),
        )?["name"],
        TextDomain::PublicName,
        Some("@fixture_original"),
        None,
        None,
    )?;
    Ok(())
}

#[test]
fn unselected_earlier_groups_cannot_own_selected_legacy_or_all_membership_links() -> TestResult {
    let filter = Filter::new("included");
    let (_, catalog) =
        catalog_crafting_support::build_from(&fixture::group_filter(), Some(&filter))?;
    let selected = catalog_crafting_support::text(fixture::SECOND_GROUP);
    assert_eq!(
        commodity::presentation(
            &catalog,
            &catalog_crafting_support::text(fixture::COMMODITY)
        )?["typeGuid"]["value"],
        selected
    );
    assert_eq!(
        *groups::parent_guid(
            &catalog,
            &catalog_crafting_support::text(fixture::CHILD_GROUP)
        )?,
        selected
    );
    assert_eq!(
        row(&catalog, "resourceTypes", fixture::RESOURCE)?["groupGuids"],
        json!([selected])
    );
    Ok(())
}

#[test]
fn a_failed_resource_dependency_retains_the_readable_commodity_and_exact_failure() -> TestResult {
    let built = catalog_crafting_support::build_diagnostic(&fixture::unreadable_resource(), None)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        commodity::presentation(
            &catalog,
            &catalog_crafting_support::text(fixture::COMMODITY)
        )?["subtypeGuid"]["value"],
        catalog_crafting_support::text(fixture::RESOURCE)
    );
    assert!(
        health::rows(&catalog, "/definitions/resourceTypes/rows")?.is_empty(),
        "the unreadable definition is not emitted as a resource row"
    );
    let references = catalog["exportHealth"]["unresolvedReferences"]
        .as_array()
        .ok_or("the unresolved references")?;
    assert!(
        references
            .iter()
            .any(|reference| reference["status"] == "targetReadFailed"
                && reference["targetRecordIndex"]["value"] == 0)
    );
    assert_eq!(catalog["exportHealth"]["status"], "incomplete");
    assert_failed_dependency_origin(&catalog)?;
    Ok(())
}

fn assert_failed_dependency_origin(catalog: &Value) -> TestResult {
    let diagnostic = health::rows(catalog, "/exportHealth/diagnostics")?
        .iter()
        .find(|row| {
            row["kind"] == "commodities"
                && row["stage"] == "derivedProjection"
                && row["code"] == "sourceReadFailure"
        })
        .ok_or("the failed resource read in the readable commodity projection")?;
    let [example] = health::rows(diagnostic, "/examples")? else {
        return Err("exactly one commodity caller caused this failure".into());
    };
    health::present(example, "recordIndex", &json!(1), Some(true))?;
    health::present_text(
        catalog,
        example,
        "field",
        &json!("Components[0].defaultComposition[0].entry"),
        Some(true),
    )?;
    health::present_text(
        catalog,
        example,
        "sourceType",
        &json!("FixtureCompositionEntry"),
        Some(true),
    )?;
    let target = health::present(
        example,
        "targetGuid",
        &json!(catalog_crafting_support::text(fixture::RESOURCE)),
        Some(true),
    )?;
    let uses = health::uses(catalog, target)?;
    let [origin] = uses.as_slice() else {
        return Err("one authored typed Reference supplies the target GUID".into());
    };
    assert_eq!(
        origin["role"], "payload",
        "the typed Reference supplies the payload"
    );
    assert_eq!(
        origin.pointer("/location/namespace"),
        Some(&json!("dataforgeValue")),
        "the payload comes from a DataForge value"
    );
    assert_eq!(
        origin.pointer("/location/indices/0"),
        Some(&json!("1")),
        "the payload belongs to the exact original Record"
    );
    assert_eq!(
        origin.pointer("/location/field"),
        Some(&json!("$value")),
        "the payload comes from the authored reference slot"
    );
    Ok(())
}

#[test]
fn all_six_thousand_original_references_allow_complete_normal_publication() -> TestResult {
    let (_, catalog) = catalog_crafting_support::build_from(&fixture::reference_budget(), None)?;
    let group = groups::members(
        &catalog,
        &catalog_crafting_support::text(fixture::FIRST_GROUP),
    )?;
    assert_eq!(
        group["resources"]["value"].as_array().map(Vec::len),
        Some(6000)
    );
    let kinds = catalog["exportHealth"]["kinds"]
        .as_array()
        .ok_or("the kind health")?;
    let group_health = kinds
        .iter()
        .find(|kind| kind["kind"] == "resourceGroups")
        .ok_or("the group health")?;
    assert_eq!(group_health["fieldInventoryComplete"], true);
    let diagnostics = catalog["exportHealth"]["diagnostics"]
        .as_array()
        .ok_or("the diagnostics")?;
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic["code"] != "fieldWalkLimit")
    );
    assert_eq!(catalog["exportHealth"]["status"], "complete");
    Ok(())
}
