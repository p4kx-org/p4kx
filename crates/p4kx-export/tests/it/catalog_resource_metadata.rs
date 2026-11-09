// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Unknown Record metadata retains failures within each required catalog source scope.

use std::error::Error;

use p4kx_export::{CatalogError, CatalogPublicationMode, Filter};
use serde_json::{Value, json};

use crate::catalog_crafting_support;
use crate::resource_edges_fixture;

type TestResult = Result<(), Box<dyn Error>>;

fn at<'a>(value: &'a Value, pointer: &str) -> Result<&'a Value, Box<dyn Error>> {
    value
        .pointer(pointer)
        .ok_or_else(|| format!("missing catalog field {pointer}").into())
}

fn health<'catalog>(
    catalog: &'catalog Value,
    kind: &str,
) -> Result<&'catalog Value, Box<dyn Error>> {
    catalog
        .pointer("/exportHealth/kinds")
        .and_then(Value::as_array)
        .and_then(|kinds| kinds.iter().find(|entry| entry["kind"] == kind))
        .ok_or_else(|| format!("the {kind} health").into())
}

#[test]
fn unreadable_type_metadata_cannot_be_guessed_as_the_first_kind_of_a_shared_prefix() -> TestResult {
    let database = resource_edges_fixture::unreadable_type_name()?;
    let Err(error) = catalog_crafting_support::build_from_data(
        database.clone(),
        None,
        CatalogPublicationMode::Normal,
    ) else {
        return Err("unknown shared-prefix metadata must block normal publication".into());
    };
    assert!(matches!(
        error.downcast_ref::<CatalogError>(),
        Some(CatalogError::IncompleteExport { .. })
    ));
    let (_, catalog) = catalog_crafting_support::build_from_data(
        database,
        None,
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    for kind in ["resourceTypes", "resourceGroups"] {
        let health = health(&catalog, kind)?;
        assert_eq!(health["sourceInventoryComplete"], false);
        assert_eq!(health["read"], "incomplete");
        assert_eq!(health["projection"], "incomplete");
    }
    let failures = at(&catalog, "/exportHealth/failedRecords")?
        .as_array()
        .ok_or("the failed Record identities")?;
    assert!(failures.iter().any(|failure| failure["recordIndex"] == 1
        && failure["kind"].is_null()
        && failure["role"] == "selectionCandidate"));
    Ok(())
}

#[test]
fn an_explicit_filter_excludes_unknown_generic_metadata_before_type_classification() -> TestResult {
    let filter = Filter::new("included");
    let database = resource_edges_fixture::unreadable_type_name()?;
    let Err(error) = catalog_crafting_support::build_from_data(
        database.clone(),
        Some(&filter),
        CatalogPublicationMode::Normal,
    ) else {
        return Err("unknown metadata still affects the unfiltered Tag taxonomy".into());
    };
    assert!(
        matches!(
            error.downcast_ref::<CatalogError>(),
            Some(CatalogError::IncompleteExport { .. })
        ),
        "unknown global Tag membership blocks normal publication"
    );
    let (_, catalog) = catalog_crafting_support::build_from_data(
        database,
        Some(&filter),
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    assert_filtered_resource_health(&catalog)?;
    assert_global_tag_uncertainty(&catalog)?;
    assert_only_global_tag_candidate_failed(&catalog)?;
    for field in [
        "uniqueSelectedSourceRecords",
        "selectedKindRecords",
        "writtenRows",
    ] {
        assert_eq!(
            catalog["exportHealth"][field], 2,
            "the unknown candidate does not inflate {field}"
        );
    }
    assert_eq!(
        catalog["exportHealth"]["publicationMode"], "diagnosticPartial",
        "the incomplete artifact requires explicit diagnostic publication"
    );
    Ok(())
}

fn assert_filtered_resource_health(catalog: &Value) -> TestResult {
    for (kind, selected, dependencies) in [
        ("resourceTypes", 1_u64, 0_u64),
        ("resourceGroups", 0, 0),
        ("commodities", 1, 1),
    ] {
        let scoped = health(catalog, kind)?;
        assert_eq!(
            scoped["sourceInventoryComplete"], true,
            "{kind} source scope"
        );
        assert_eq!(scoped["fieldInventoryComplete"], true, "{kind} field scope");
        assert_eq!(scoped["read"], "complete", "{kind} canonical reads");
        assert_eq!(scoped["projection"], "complete", "{kind} projections");
        assert_eq!(
            scoped["sourceRecords"],
            json!({
                "selected": selected,
                "readSuccessful": selected,
                "projected": selected,
                "intentionallySkipped": 0,
                "omittedAfterReadFailure": 0,
                "omittedAfterProjectionFailure": 0,
                "dependencies": dependencies,
                "failedDependencies": 0,
                "failedSelectionCandidates": 0
            }),
            "{kind} retains every exact scoped source counter without the excluded candidate"
        );
        assert_eq!(scoped["writtenRows"], selected, "{kind} emitted rows");
        assert_eq!(
            scoped["failedRecords"],
            json!([]),
            "{kind} has no failed source"
        );
        assert_eq!(
            at(catalog, &format!("/completeness/{kind}"))?,
            &json!({"status": "partial", "filter": "included"}),
            "{kind} retains its explicit Entity Filter scope"
        );
    }
    Ok(())
}

fn assert_global_tag_uncertainty(catalog: &Value) -> TestResult {
    let tags = health(catalog, "tags")?;
    assert_eq!(
        tags["sourceInventoryComplete"], false,
        "unknown Tag membership"
    );
    assert_eq!(
        tags["read"], "incomplete",
        "global metadata remains unreadable"
    );
    assert_eq!(tags["projection"], "incomplete", "no complete Tag claim");
    assert_eq!(
        tags["sourceRecords"],
        json!({
            "selected": 0,
            "readSuccessful": 0,
            "projected": 0,
            "intentionallySkipped": 0,
            "omittedAfterReadFailure": 0,
            "omittedAfterProjectionFailure": 0,
            "dependencies": 0,
            "failedDependencies": 0,
            "failedSelectionCandidates": 1
        }),
        "unknown membership remains one candidate without a confirmed, skipped or dependent Tag"
    );
    assert_eq!(tags["writtenRows"], 0, "no fabricated Tag definition");
    assert_eq!(
        at(catalog, "/exportHealth/status")?,
        &json!("incomplete"),
        "scoped resource health cannot erase global selection uncertainty"
    );
    assert_eq!(
        at(catalog, "/completeness/tags")?,
        &json!({"status": "complete"}),
        "full selection scope remains separate from successful reads"
    );
    assert_eq!(
        at(catalog, "/definitions/tags/availability")?,
        &json!("unavailable"),
        "unreadable type metadata cannot prove that the Tag family is absent"
    );
    assert_eq!(
        at(catalog, "/definitions/tags/rows")?,
        &json!([]),
        "an empty unavailable table retains its blocking health evidence"
    );
    Ok(())
}

fn assert_only_global_tag_candidate_failed(catalog: &Value) -> TestResult {
    let failures = at(catalog, "/exportHealth/failedRecords")?
        .as_array()
        .ok_or("the failed Record identities")?;
    assert_eq!(
        failures.len(),
        1,
        "only one affected global selection source"
    );
    let candidate = failures.first().ok_or("the unknown Tag candidate")?;
    for (field, expected) in [
        ("kind", json!("tags")),
        ("recordIndex", json!(1)),
        (
            "guid",
            json!(catalog_crafting_support::text(
                resource_edges_fixture::MISSING
            )),
        ),
        (
            "recordPath",
            json!("libs/foundry/records/resourcetypedatabase/excluded/metadata"),
        ),
        ("role", json!("selectionCandidate")),
        ("stage", json!("selection")),
        ("code", json!("selectionFailure")),
    ] {
        assert_eq!(
            candidate[field], expected,
            "the global failure retains its exact {field} without an unscoped substitute"
        );
    }
    Ok(())
}

#[test]
fn an_invalid_record_struct_index_is_rejected_before_either_publication_mode() -> TestResult {
    for mode in [
        CatalogPublicationMode::Normal,
        CatalogPublicationMode::DiagnosticPartial,
    ] {
        let Err(error) = catalog_crafting_support::build_from_data(
            resource_edges_fixture::unreadable_type_metadata().build(),
            None,
            mode,
        ) else {
            return Err("an invalid Record struct index cannot open a DataForge".into());
        };
        assert!(matches!(
            error.downcast_ref::<CatalogError>(),
            Some(CatalogError::DataForge(
                p4kx_extraction::DataForgeError::IndexOutOfRange {
                    part: "Record",
                    index: 1,
                    target: "struct",
                    ..
                }
            ))
        ));
    }
    Ok(())
}
