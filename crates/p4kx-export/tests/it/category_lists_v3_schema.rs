// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogPublicationMode;
use serde_json::{Value, json};

use crate::category_lists_v3_fixture as fixture;
use crate::category_lists_v3_source as source;
use crate::commodity_v3_fixture::{DcbOrigin, TestResult};

#[test]
fn category_lists_v3_schema_requires_nested_carriers_and_closed_reference_controls() -> TestResult {
    let database = fixture::clean();
    source::ordered(&database, false)?;
    let (_, catalog) = fixture::build(
        &database,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        None,
    )?;
    fixture::row(&catalog, 0)?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    assert!(
        validator.is_valid(&catalog),
        "the actual source-backed list export satisfies its generated schema"
    );
    for carrier in [
        "/declarations/blueprintCategoryLists/rows/0/recordName",
        "/declarations/blueprintCategoryLists/rows/0/categories",
        "/declarations/blueprintCategoryLists/rows/0/categories/value/0",
        "/declarations/blueprintCategoryLists/rows/0/categories/value/0/value/guid",
        "/declarations/blueprintCategoryLists/rows/0/categories/value/0/value/targetRecordIndex",
    ] {
        for key in [
            "state",
            "value",
            "sourceUsesRefs",
            "sourceSetComplete",
            "containsEncryptedSource",
        ] {
            let mut invalid = catalog.clone();
            invalid
                .pointer_mut(carrier)
                .and_then(Value::as_object_mut)
                .ok_or("original nested carrier")?
                .remove(key)
                .ok_or("required source key")?;
            assert!(
                !validator.is_valid(&invalid),
                "each immediate carrier requires {key} at {carrier}"
            );
        }
    }
    let mut invalid = catalog;
    *invalid
        .pointer_mut("/declarations/blueprintCategoryLists/rows/0/categories/value/0/value/status")
        .ok_or("original reference status")? = json!("futureResolution");
    assert!(
        !validator.is_valid(&invalid),
        "source resolution remains a closed control instead of accepting an unknown success state"
    );
    Ok(())
}
