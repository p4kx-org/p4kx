// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::json;

use crate::catalog_schema_support::minimal_catalog;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn health_and_its_publication_policy_are_required_in_the_file() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let catalog = minimal_catalog();
    assert!(
        validator.is_valid(&catalog),
        "the structural diagnostic fixture is valid"
    );
    for field in [
        "status",
        "read",
        "projection",
        "publicationMode",
        "kinds",
        "failedRecords",
        "failedEntries",
        "unresolvedReferences",
        "diagnostics",
    ] {
        let mut missing = catalog.clone();
        missing["exportHealth"]
            .as_object_mut()
            .ok_or("health object")?
            .remove(field);
        assert!(
            !validator.is_valid(&missing),
            "missing health field {field} cannot validate"
        );
    }
    let mut missing = catalog;
    missing
        .as_object_mut()
        .ok_or("catalog object")?
        .remove("exportHealth");
    assert!(
        !validator.is_valid(&missing),
        "a copied file must carry health"
    );
    Ok(())
}

#[test]
fn record_and_entry_counters_remain_distinct_nonnegative_integer_units() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (unit, field) in [
        ("sourceRecords", "omittedAfterReadFailure"),
        ("sourceEntries", "failedRequiredReads"),
    ] {
        for value in [json!(-1), json!(0.5), json!("0"), json!(null)] {
            let mut invalid = minimal_catalog();
            invalid["exportHealth"]["kinds"][0][unit][field] = value;
            assert!(
                !validator.is_valid(&invalid),
                "{unit}.{field} is a nonnegative integer count"
            );
        }
        let mut missing = minimal_catalog();
        missing["exportHealth"]["kinds"][0]
            .as_object_mut()
            .ok_or("kind health object")?
            .remove(unit);
        assert!(!validator.is_valid(&missing), "{unit} cannot disappear");
    }
    Ok(())
}

#[test]
fn nullable_diagnostic_metadata_is_present_even_when_unknown() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let mut catalog = minimal_catalog();
    catalog["exportHealth"]["diagnostics"][0]["warningCode"] = json!(null);
    assert!(
        validator.is_valid(&catalog),
        "an unknown warning code is explicit null"
    );
    catalog["exportHealth"]["diagnostics"][0]
        .as_object_mut()
        .ok_or("diagnostic object")?
        .remove("warningCode");
    assert!(
        !validator.is_valid(&catalog),
        "an omitted nullable member violates the contract"
    );
    Ok(())
}

#[test]
fn future_health_codes_and_kind_names_remain_additive_strings() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let mut catalog = minimal_catalog();
    catalog["exportHealth"]["diagnostics"][0]["code"] = json!("futureSourceDiagnostic");
    catalog["exportHealth"]["kinds"][0]["kind"] = json!("futureCatalogKind");
    assert!(
        validator.is_valid(&catalog),
        "future codes and kinds are additive"
    );
    catalog["exportHealth"]["diagnostics"][0]["code"] = json!(1);
    assert!(
        !validator.is_valid(&catalog),
        "the open code set remains string-typed"
    );
    Ok(())
}
