// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::CatalogPublicationMode;
use p4kx_extraction::{DataForge, DataForgeError, Value as Raw};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};
use serde_json::{Value, json};

use crate::blueprint_category_v3_fixture as categories;
use crate::category_lists_v3_fixture as fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::commodity_v3_source_inventory as inventory;
use crate::crafting_fixture;

#[test]
fn category_lists_v3_keeps_a_foreign_catalog_family_as_a_raw_source_reference() -> TestResult {
    let target = 0xfe20;
    let database = fixture::schema()
        .structure(TestStruct::new("ResourceType"))
        .record(fixture::listing(
            fixture::LIST,
            "Primary",
            fixture::LIST_FILE,
            fixture::references(&[target]),
        ))
        .record(TestRecord::new(
            target,
            "ResourceType",
            "ResourceType.Foreign",
            "synthetic/foreign/Resource.XML",
        ));
    assert_foreign_source(&database, target)?;
    let (built, catalog) = fixture::build(
        &database,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        None,
    )?;
    fixture::count(&built, 1, 0)?;
    let slot = fixture::at(fixture::row(&catalog, 0)?, "/categories/value/0/value")?;
    for (path, expected) in [
        ("/guid/value", json!(common::guid(target))),
        ("/targetRecordIndex/value", json!(1)),
        ("/targetType/value", json!("ResourceType")),
        ("/recordPath/value", json!("synthetic/foreign/Resource")),
        ("/status", json!("present")),
    ] {
        assert_eq!(
            fixture::at(slot, path)?,
            &expected,
            "raw foreign facts survive"
        );
    }
    assert_raw_reference_origins(&catalog, slot)?;
    assert_eq!(
        fixture::at(&catalog, "/exportHealth/unresolvedReferences")?,
        &json!([]),
        "the raw declaration adds no catalog-materialization join"
    );
    for path in [
        "/definitions/resourceTypes/rows",
        "/definitions/blueprintCategories/rows",
    ] {
        assert_eq!(
            fixture::at(&catalog, path)?,
            &json!([]),
            "no target backfill"
        );
    }
    assert_eq!(
        fixture::at(&catalog, "/exportHealth/status")?,
        &json!("complete"),
        "a format-valid raw reference adds no failure"
    );
    Ok(())
}

fn assert_foreign_source(database: &DataForgeBuilder, guid: u128) -> TestResult {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(0).ok_or("original list owner")?.tree()?;
    let Some(Raw::Array(values)) = tree.root().field("categories") else {
        return Err("original authored array".into());
    };
    let Some(Raw::Reference(reference)) = values.first() else {
        return Err("actual resolved foreign reference".into());
    };
    assert_eq!(
        reference.guid().to_string(),
        common::guid(guid),
        "original token"
    );
    assert_eq!(reference.record_index(), 1, "original physical target");
    assert_eq!(reference.type_name(), "ResourceType", "real catalog family");
    assert_eq!(
        reference.record_path(),
        "synthetic/foreign/Resource",
        "the physical target is outside Resource selection"
    );
    Ok(())
}

fn assert_raw_reference_origins(catalog: &Value, slot: &Value) -> TestResult {
    assert_eq!(
        inventory::uses(catalog, fixture::at(slot, "/guid")?)?,
        BTreeSet::from([(0, inventory::body("categories[0]"), "payload".to_owned())]),
        "authored GUID remains the original list body fact"
    );
    for (field, input, role) in [
        ("targetRecordIndex", "recordIndex", "selector"),
        ("targetType", "sourceType", "selector"),
        ("recordPath", "fileName", "conversion"),
    ] {
        assert_eq!(
            inventory::uses(catalog, fixture::at(slot, &format!("/{field}"))?)?,
            BTreeSet::from([(1, inventory::header(input), role.to_owned())]),
            "only the actual foreign target header contributes"
        );
    }
    Ok(())
}

#[test]
fn category_lists_v3_read_failure_cannot_unlink_a_valid_same_guid_category() -> TestResult {
    let database = crafting_fixture::database()
        .structure(fixture::list_schema().declared_size(9))
        .record(fixture::listing(
            crafting_fixture::CATEGORY,
            "AliasFailure",
            "synthetic/category_lists/AliasFailure.XML",
            fixture::references(&[]),
        ));
    let failed_index = assert_alias_source(&database)?;
    let (_, catalog) = fixture::build(
        &database,
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        None,
    )?;
    assert_alias_failure(&catalog, failed_index)?;
    let guid = common::guid(crafting_fixture::CATEGORY);
    let definitions = categories::rows(&catalog)?;
    assert!(
        definitions.iter().any(|row| {
            row.pointer("/guid/value").and_then(Value::as_str) == Some(guid.as_str())
        }),
        "the valid original category definition remains present"
    );
    let blueprint =
        crate::catalog_crafting_support::blueprint::by_guid(&catalog, crafting_fixture::MAGAZINE)?;
    assert_eq!(
        blueprint.presentation["category"]["value"], guid,
        "an unrelated list read failure cannot clear the actual category link"
    );
    Ok(())
}

fn assert_alias_source(database: &DataForgeBuilder) -> TestResult<u32> {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let guid = common::guid(crafting_fixture::CATEGORY);
    let mut category = None;
    let mut failed = None;
    for record in forge.records() {
        if record.guid().to_string() != guid {
            continue;
        }
        match record.type_name()? {
            fixture::CATEGORY_TYPE => {
                assert!(
                    record.tree()?.root().fields().is_empty(),
                    "valid fieldless category"
                );
                category = Some(record.index());
            }
            fixture::TYPE => {
                assert!(
                    matches!(
                        record.tree(),
                        Err(DataForgeError::StructLayout {
                            declared: 9,
                            fields: 8,
                            ..
                        })
                    ),
                    "the distinct original database has the intended strict failure"
                );
                failed = Some(record.index());
            }
            _ => return Err("unexpected GUID alias source".into()),
        }
    }
    let category = category.ok_or("actual valid category")?;
    let failed = failed.ok_or("actual broken database alias")?;
    assert_ne!(
        category, failed,
        "same nominal GUID means distinct physical owners"
    );
    assert_original_blueprint(&forge, category)?;
    Ok(failed)
}

fn assert_original_blueprint(forge: &DataForge<'_>, category: u32) -> TestResult {
    let guid = common::guid(crafting_fixture::MAGAZINE);
    let record = forge
        .records()
        .find(|record| record.guid().to_string() == guid)
        .ok_or("original blueprint owner")?;
    let tree = record.tree()?;
    let Some(Raw::Struct(blueprint)) = tree.root().field("blueprint") else {
        return Err("original blueprint body".into());
    };
    let Some(Raw::Reference(reference)) = blueprint.field("category") else {
        return Err("original category link".into());
    };
    assert_eq!(
        reference.record_index(),
        category,
        "the actual link selects the valid category"
    );
    assert_eq!(
        reference.type_name(),
        fixture::CATEGORY_TYPE,
        "not the database alias"
    );
    Ok(())
}

fn assert_alias_failure(catalog: &Value, index: u32) -> TestResult {
    let guid = common::guid(crafting_fixture::CATEGORY);
    let failed = fixture::at(catalog, "/exportHealth/failedRecords")?
        .as_array()
        .ok_or("actual source failures")?;
    assert!(
        failed.iter().any(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(index))
                && row.pointer("/guid/value").and_then(Value::as_str) == Some(guid.as_str())
                && row.get("kind").and_then(Value::as_str) == Some(fixture::KIND)
                && row.get("role").and_then(Value::as_str) == Some("selected")
                && row.get("code").and_then(Value::as_str) == Some("sourceReadFailure")
        }),
        "the actual failed owner is the database, not the valid category"
    );
    assert!(
        !failed.iter().any(|row| {
            row.pointer("/guid/value").and_then(Value::as_str) == Some(guid.as_str())
                && row.get("kind").and_then(Value::as_str) == Some("blueprintCategories")
        }),
        "a database failure cannot fabricate a category-definition failure"
    );
    Ok(())
}
