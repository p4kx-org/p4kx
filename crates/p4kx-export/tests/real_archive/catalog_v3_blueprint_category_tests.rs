// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_testkit::{TestRecord, TestStruct};
use serde_json::{Value, json};

use crate::catalog_v3_consumer::{field, identity, rows};
use crate::commodity_v3_fixture::{self as fixture, TestResult};

const GUID: u128 = 0x8d01;
const KIND: Kind = Kind::BlueprintCategories;
const TABLE: &str = "/definitions/blueprintCategories";
const FIRST: &str = "/definitions/blueprintCategories/rows/0";
const HEADER: [&str; 8] = [
    "guid",
    "recordName",
    "className",
    "recordPath",
    "recordIndex",
    "sourceType",
    "fileName",
    "metadataTag",
];

fn catalog() -> TestResult<Value> {
    let database = fixture::database()
        .structure(TestStruct::new("BlueprintCategoryRecord"))
        .record(category(GUID, "First"))
        .record(category(GUID, "Second"))
        .record(category(0x8d00, "Lower"));
    Ok(fixture::build(&database)?.1)
}

fn category(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "BlueprintCategoryRecord",
        format!("BlueprintCategoryRecord.{name}"),
        format!("libs/foundry/records/crafting/blueprintcategories/{name}.xml"),
    )
}

#[test]
fn canonical_category_rows_ignore_a_legacy_distraction_and_require_the_real_table() -> TestResult {
    let mut catalog = catalog()?;
    assert_eq!(
        rows(&catalog, KIND)?.len(),
        3,
        "the actual producer retains every selected physical category"
    );
    catalog.as_object_mut().ok_or("catalog object")?.insert(
        "blueprintCategories".to_owned(),
        json!([{"guid":"legacy-distraction"}]),
    );
    assert_eq!(
        rows(&catalog, KIND)?.len(),
        3,
        "a retired root never becomes a canonical row source"
    );
    catalog
        .get_mut("definitions")
        .and_then(Value::as_object_mut)
        .ok_or("definitions")?
        .remove("blueprintCategories")
        .ok_or("canonical category table")?;
    assert!(
        rows(&catalog, KIND).is_err(),
        "a missing required table is not an empty success"
    );
    Ok(())
}

#[test]
fn category_readers_require_all_eight_source_cells_and_present_guid_identity() -> TestResult {
    let catalog = catalog()?;
    for name in HEADER {
        let mut changed = catalog.clone();
        changed
            .pointer_mut(FIRST)
            .and_then(Value::as_object_mut)
            .ok_or("category row")?
            .remove(name)
            .ok_or("original header cell")?;
        assert!(
            rows(&changed, KIND).is_err(),
            "missing {name} cannot silently disappear"
        );
    }
    for state in [
        "absent",
        "null",
        "unreadable",
        "unsupported",
        "invalid",
        "unknown",
    ] {
        let mut changed = catalog.clone();
        let guid = changed
            .pointer_mut(&format!("{FIRST}/guid"))
            .and_then(Value::as_object_mut)
            .ok_or("GUID carrier")?;
        guid.insert("state".to_owned(), json!(state));
        guid.insert("value".to_owned(), Value::Null);
        assert!(
            rows(&changed, KIND).is_err(),
            "a {state} identity is not a readable definition"
        );
    }
    Ok(())
}

#[test]
fn category_identity_is_physical_and_does_not_require_unique_nominal_guids() -> TestResult {
    let catalog = catalog()?;
    let actual = rows(&catalog, KIND)?;
    let duplicate = fixture::guid(GUID);
    let mut alternatives = 0;
    for row in actual {
        if identity(row, KIND)? == duplicate {
            alternatives += 1;
        }
    }
    assert_eq!(
        alternatives, 2,
        "same-GUID source alternatives are preserved rather than deduplicated"
    );
    crate::real_archive_verification::check_unique_guids(&catalog)?;
    let mut repeated = catalog.clone();
    let row = actual.last().ok_or("actual last category")?.clone();
    repeated
        .pointer_mut(&format!("{TABLE}/rows"))
        .and_then(Value::as_array_mut)
        .ok_or("category rows")?
        .push(row);
    assert!(
        crate::real_archive_verification::check_unique_guids(&repeated).is_err(),
        "the same physical original definition cannot be emitted twice"
    );
    let mut wrong_family = catalog.clone();
    *wrong_family
        .pointer_mut(&format!("{FIRST}/sourceType/value"))
        .ok_or("source type")? = json!("BlueprintCategoryDatabaseRecord");
    assert!(
        crate::real_archive_verification::check_unique_guids(&wrong_family).is_err(),
        "a database listing is not a category definition"
    );
    Ok(())
}

#[test]
fn category_header_values_keep_types_nullable_declarations_and_nil_guids() -> TestResult {
    let catalog = catalog()?;
    let mut wrong_index = catalog.clone();
    *wrong_index
        .pointer_mut(&format!("{FIRST}/recordIndex/value"))
        .ok_or("physical index")? = json!("0");
    assert!(
        rows(&wrong_index, KIND).is_err(),
        "a physical Record index is a typed source number"
    );
    let mut nullable = catalog.clone();
    for name in ["recordName", "className", "metadataTag"] {
        let carrier = nullable
            .pointer_mut(&format!("{FIRST}/{name}"))
            .and_then(Value::as_object_mut)
            .ok_or("nullable declaration")?;
        carrier.insert("state".to_owned(), json!("null"));
        carrier.insert("value".to_owned(), Value::Null);
    }
    assert_eq!(
        rows(&nullable, KIND)?.len(),
        3,
        "a declared null header text is retained without inventing an empty string"
    );
    let mut nil = catalog;
    let row = nil.pointer_mut(FIRST).ok_or("actual category")?;
    *row.pointer_mut("/guid/value").ok_or("GUID value")? =
        json!("00000000-0000-0000-0000-000000000000");
    assert_eq!(
        identity(row, KIND)?,
        "00000000-0000-0000-0000-000000000000",
        "canonical GUID parsing adds no unrelated non-nil rule"
    );
    assert!(
        field(row, KIND, "recordIndex")?.is_number(),
        "the physical source position remains a present numeric cell"
    );
    Ok(())
}
