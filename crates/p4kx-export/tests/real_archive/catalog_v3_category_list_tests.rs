// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_extraction::{DataForge, Guid, Value as Raw};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::check_original_lists;
use crate::catalog_v3_consumer as reader;
use crate::commodity_v3_fixture::{self as fixture, TestResult};

const KIND: Kind = Kind::BlueprintCategoryLists;
const TYPE: &str = "BlueprintCategoryDatabaseRecord";
const GUID: u128 = 0x9d01;
const TARGET: u128 = 0x9d02;
const TABLE: &str = "/declarations/blueprintCategoryLists";
const FIRST: &str = "/declarations/blueprintCategoryLists/rows/0";

fn database() -> DataForgeBuilder {
    fixture::database()
        .structure(TestStruct::new("BlueprintCategoryRecord"))
        .structure(TestStruct::new(TYPE).array(
            "categories",
            TestType::ReferenceTo("BlueprintCategoryRecord".to_owned()),
        ))
        .record(listing("First"))
        .record(
            TestRecord::new(
                TARGET,
                "BlueprintCategoryRecord",
                "BlueprintCategoryRecord.Ammo",
                "libs/foundry/records/crafting/blueprintcategories/Ammo.XML",
            )
            .tag("CategoryDomain"),
        )
        .record(listing("Second"))
}

fn listing(name: &str) -> TestRecord {
    TestRecord::new(
        GUID,
        TYPE,
        format!("{TYPE}.{name}"),
        format!("synthetic/category_lists/{name}.XML"),
    )
    .tag(format!("{name}Domain"))
    .value(
        "categories",
        TestValue::Array(vec![
            TestValue::Reference(TARGET),
            TestValue::Null,
            TestValue::Reference(TARGET),
        ]),
    )
}

fn original_source(database: &DataForgeBuilder) -> TestResult {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let mut count = 0_usize;
    for record in forge.records() {
        if record.type_name()? == TYPE {
            assert_eq!(
                record.guid().to_string(),
                fixture::guid(GUID),
                "the source has two nominally equal but physically distinct lists"
            );
            let tree = record.tree()?;
            let Some(Raw::Array(slots)) = tree.root().field("categories") else {
                return Err("original ordered reference array".into());
            };
            assert_eq!(
                slots.len(),
                3,
                "the source really contains every repeated/null slot"
            );
            assert!(
                matches!(slots.get(1), Some(Raw::Null)),
                "the original middle slot is null"
            );
            for ordinal in [0, 2] {
                let Some(Raw::Reference(target)) = slots.get(ordinal) else {
                    return Err("original resolved repeated reference".into());
                };
                assert_eq!(
                    target.guid().to_string(),
                    fixture::guid(TARGET),
                    "both original slots select the actual category target"
                );
            }
            count += 1;
        }
    }
    assert_eq!(
        count, 2,
        "the original source supplies exactly both physical alternatives"
    );
    Ok(())
}

#[test]
fn canonical_lists_consumer_uses_the_actual_table_and_all_original_header_cells() -> TestResult {
    let database = database();
    original_source(&database)?;
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let (_, catalog) = fixture::build(&database)?;
    check_original_lists(&catalog, &forge)?;
    assert_eq!(
        reader::rows(&catalog, KIND)?.len(),
        2,
        "both original physical rows reach the canonical consumer"
    );
    let mut distraction = catalog.clone();
    distraction.as_object_mut().ok_or("catalog")?.insert(
        "blueprintCategoryLists".to_owned(),
        json!([{"guid":"retired-flat-distraction"}]),
    );
    assert_eq!(
        reader::rows(&distraction, KIND)?.len(),
        2,
        "the flat view is ignored"
    );
    distraction
        .get_mut("declarations")
        .and_then(Value::as_object_mut)
        .ok_or("declarations")?
        .remove("blueprintCategoryLists")
        .ok_or("actual A table")?;
    assert!(
        reader::rows(&distraction, KIND).is_err(),
        "a missing table is never an empty success"
    );
    assert_original_header_cells(&catalog, &forge)?;
    Ok(())
}

fn assert_original_header_cells(catalog: &Value, forge: &DataForge) -> TestResult {
    for field in [
        "guid",
        "recordName",
        "className",
        "recordPath",
        "recordIndex",
        "sourceType",
        "fileName",
        "metadataTag",
    ] {
        let mut missing = catalog.clone();
        missing
            .pointer_mut(FIRST)
            .and_then(Value::as_object_mut)
            .ok_or("original row")?
            .remove(field)
            .ok_or("required original header")?;
        assert!(
            reader::rows(&missing, KIND).is_err(),
            "the immediate {field} carrier is required"
        );
        let mut altered = catalog.clone();
        *altered
            .pointer_mut(&format!("{FIRST}/{field}/value"))
            .ok_or("original header value")? = if field == "recordIndex" {
            json!(0)
        } else {
            json!("Changed")
        };
        assert!(
            check_original_lists(&altered, forge).is_err(),
            "the original metadata oracle rejects a changed {field}, even if its carrier remains valid"
        );
    }
    Ok(())
}

#[test]
fn canonical_lists_consumer_retains_slots_and_rejects_missing_origins_or_unknown_controls()
-> TestResult {
    let database = database();
    original_source(&database)?;
    let (_, catalog) = fixture::build(&database)?;
    let slot = catalog
        .pointer(&format!("{FIRST}/categories/value"))
        .and_then(Value::as_array)
        .ok_or("actual ordered source slots")?;
    assert_eq!(slot.len(), 3, "repeated and null slots retain cardinality");
    assert_eq!(
        slot.get(1).and_then(|slot| slot.get("state")),
        Some(&json!("null")),
        "the original null slot remains at its ordinal"
    );
    for pointer in [
        format!("{FIRST}/categories"),
        format!("{FIRST}/categories/value/0"),
        format!("{FIRST}/categories/value/0/value/guid"),
        format!("{FIRST}/categories/value/0/value/targetRecordIndex"),
        format!("{FIRST}/categories/value/0/value/targetType"),
        format!("{FIRST}/categories/value/0/value/recordPath"),
    ] {
        let mut missing = catalog.clone();
        missing
            .pointer_mut(&pointer)
            .and_then(Value::as_object_mut)
            .ok_or("actual immediate data cell")?
            .remove("sourceUsesRefs")
            .ok_or("required uses")?;
        assert!(
            reader::rows(&missing, KIND).is_err(),
            "each original nested source cell requires its origins"
        );
    }
    let mut unknown = catalog.clone();
    *unknown
        .pointer_mut(&format!("{FIRST}/categories/value/0/value/status"))
        .ok_or("actual status")? = json!("future-success");
    assert!(
        reader::rows(&unknown, KIND).is_err(),
        "unknown resolution cannot grant successful interpretation"
    );
    let mut scalar = catalog.clone();
    *scalar
        .pointer_mut(&format!("{FIRST}/categories/value"))
        .ok_or("actual array")? = json!("not-an-array");
    assert!(
        reader::rows(&scalar, KIND).is_err(),
        "the ordered source array retains its typed shape"
    );
    Ok(())
}

fn original_quantization_index(database: &DataForgeBuilder) -> TestResult<u32> {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let expected: Guid = fixture::guid(fixture::QUANTIZATION).parse()?;
    let mut found = None;
    for (ordinal, record) in forge.records().enumerate() {
        if record.guid() == expected {
            assert!(
                found.is_none(),
                "the original fixture has one physical quantizer owner"
            );
            assert_eq!(
                record.type_name()?,
                "CraftingQualityQuantizationRecord",
                "the original matching GUID belongs to the Quality source family"
            );
            assert_eq!(
                usize::try_from(record.index())?,
                ordinal,
                "the matching quantizer retains its original Record-table position"
            );
            found = Some(record.index());
        }
    }
    found.ok_or_else(|| "the original source must contain the expected quantizer GUID".into())
}

fn assert_original_quantization(catalog: &Value, original_index: u32) -> TestResult {
    let kind = Kind::QualityQuantizations;
    let expected = fixture::guid(fixture::QUANTIZATION);
    let mut count = 0_usize;
    for row in reader::rows(catalog, kind)? {
        if reader::identity(row, kind)? == expected {
            assert_eq!(
                reader::field(row, kind, "recordIndex")?,
                &json!(original_index),
                "the actual canonical quantizer retains its original physical owner"
            );
            assert_eq!(
                reader::field(row, kind, "sourceType")?.as_str(),
                Some("CraftingQualityQuantizationRecord"),
                "the actual canonical collision target remains a Quality definition"
            );
            count += 1;
        }
    }
    assert_eq!(
        count, 1,
        "the actual canonical dataset contains the original quantizer once"
    );
    Ok(())
}

#[test]
fn canonical_lists_physical_alternatives_do_not_waive_cross_family_identity() -> TestResult {
    let database = database();
    original_source(&database)?;
    let quality_index = original_quantization_index(&database)?;
    let (_, catalog) = fixture::build(&database)?;
    assert_original_quantization(&catalog, quality_index)?;
    crate::real_archive_verification::check_unique_guids(&catalog)?;
    let mut repeated = catalog.clone();
    let row = reader::rows(&catalog, KIND)?
        .first()
        .ok_or("actual physical row")?
        .clone();
    repeated
        .pointer_mut(&format!("{TABLE}/rows"))
        .and_then(Value::as_array_mut)
        .ok_or("canonical list rows")?
        .push(row);
    assert!(
        crate::real_archive_verification::check_unique_guids(&repeated).is_err(),
        "one original physical row cannot be retained twice"
    );
    for guid in [fixture::RESOURCE, TARGET, fixture::QUANTIZATION] {
        let mut alias = catalog.clone();
        for row in alias
            .pointer_mut(&format!("{TABLE}/rows"))
            .and_then(Value::as_array_mut)
            .ok_or("actual list alternatives")?
        {
            *row.pointer_mut("/guid/value")
                .ok_or("actual original list GUID")? = json!(fixture::guid(guid));
        }
        super::check(&alias, KIND)?;
        assert!(
            crate::real_archive_verification::check_unique_guids(&alias).is_err(),
            "Resource, Category and Quality definitions cannot share the list-family allowance"
        );
    }
    let mut wrong = catalog;
    *wrong
        .pointer_mut(&format!("{FIRST}/sourceType/value"))
        .ok_or("actual original type")? = json!("BlueprintCategoryRecord");
    assert!(
        crate::real_archive_verification::check_unique_guids(&wrong).is_err(),
        "a category definition cannot impersonate a database physical alternative"
    );
    Ok(())
}
