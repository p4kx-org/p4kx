// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Unknown Record types cannot prove a complete shared resource inventory.

use std::error::Error;

use p4kx_export::{CatalogError, CatalogPublicationMode, Filter};
use serde_json::Value;

use crate::catalog_crafting_support;
use crate::resource_edges_fixture;

type TestResult = Result<(), Box<dyn Error>>;

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
    let failures = catalog["exportHealth"]["failedRecords"]
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
    let (_, catalog) = catalog_crafting_support::build_from_data(
        resource_edges_fixture::unreadable_type_name()?,
        Some(&filter),
        CatalogPublicationMode::Normal,
    )?;
    for kind in ["resourceTypes", "resourceGroups", "commodities"] {
        let health = health(&catalog, kind)?;
        assert_eq!(health["sourceInventoryComplete"], true, "{kind}");
        assert_eq!(health["read"], "complete", "{kind}");
        assert_eq!(health["projection"], "complete", "{kind}");
    }
    let failures = catalog["exportHealth"]["failedRecords"]
        .as_array()
        .ok_or("the failed Record identities")?;
    assert!(
        !failures
            .iter()
            .any(|failure| failure["recordIndex"] == 1 && failure["kind"].is_null())
    );
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
