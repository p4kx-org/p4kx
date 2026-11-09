// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter, Kind};
use p4kx_extraction::{DataForge, Value as SourceValue};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::json;

use crate::blueprint_category_v3_fixture as fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn category_v3_preserves_kind_filter_leftover_and_database_exclusion_rules() -> TestResult {
    let second = "libs/foundry/records/crafting/blueprintcategories/included/Armour.XML";
    let leftover = "libs/foundry/records/crafting/blueprintcategories/included/Unused.XML";
    let database = "libs/foundry/records/crafting/blueprintcategories/included/Database.XML";
    let source = fixture::single()
        .structure(TestStruct::new("BlueprintCategoryDatabaseRecord").array(
            "categories",
            TestType::ReferenceTo(fixture::SOURCE_TYPE.to_owned()),
        ))
        .record(fixture::category(
            fixture::LOWER,
            "BlueprintCategoryRecord.Armour",
            second,
        ))
        .record(fixture::category(
            fixture::LEFTOVER,
            "BlueprintCategoryRecord.Test_Category",
            leftover,
        ))
        .record(
            TestRecord::new(
                fixture::DATABASE,
                "BlueprintCategoryDatabaseRecord",
                "BlueprintCategoryDatabaseRecord.Listing",
                database,
            )
            .value(
                "categories",
                TestValue::Array(vec![
                    TestValue::Reference(fixture::CATEGORY),
                    TestValue::Reference(fixture::LOWER),
                ]),
            ),
        );
    fixture::original(
        &source,
        0,
        fixture::CATEGORY,
        fixture::RAW_NAME,
        fixture::FILE_NAME,
        fixture::TAG,
    )?;
    fixture::original(
        &source,
        1,
        fixture::LOWER,
        "BlueprintCategoryRecord.Armour",
        second,
        fixture::TAG,
    )?;
    fixture::original(
        &source,
        2,
        fixture::LEFTOVER,
        "BlueprintCategoryRecord.Test_Category",
        leftover,
        fixture::TAG,
    )?;
    assert_original_database(&source)?;
    let filter = Filter::new("*/included/*");
    for selected in [None, Some(&filter)] {
        let (built, catalog) = fixture::build(
            &source,
            DcbOrigin::Plain,
            CatalogPublicationMode::Normal,
            selected,
        )?;
        fixture::counts(&built, 2, 1)?;
        assert_eq!(
            built
                .summary()
                .kind(Kind::BlueprintCategories)
                .map(|kind| kind.complete),
            Some(selected.is_none()),
            "Filter completeness keeps the existing independent selection meaning"
        );
        let indices = fixture::rows(&catalog)?
            .iter()
            .map(|row| {
                fixture::at(row, "/recordIndex/value")?
                    .as_u64()
                    .ok_or_else(|| "category index".into())
            })
            .collect::<TestResult<Vec<_>>>()?;
        assert_eq!(
            indices,
            [1, 0],
            "only actual category Records survive, in GUID order"
        );
        assert_eq!(
            fixture::at(&catalog, "/definitions/blueprintCategories/availability")?,
            &json!("available"),
            "the family remains declared"
        );
    }
    Ok(())
}

fn assert_original_database(source: &DataForgeBuilder) -> TestResult {
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(3).ok_or("original category database")?;
    assert_eq!(record.index(), 3, "database has its own physical owner");
    assert_eq!(
        record.guid().to_string(),
        common::guid(fixture::DATABASE),
        "database identity"
    );
    assert_eq!(
        record.type_name()?,
        "BlueprintCategoryDatabaseRecord",
        "database is not a category Record"
    );
    let tree = record.tree()?;
    let Some(SourceValue::Array(entries)) = tree.root().field("categories") else {
        return Err("the original separate database listing must really be populated".into());
    };
    let targets = entries
        .iter()
        .map(|value| match value {
            SourceValue::Reference(reference) => Ok(reference.guid().to_string()),
            _ => Err("original database slot must be a Reference".into()),
        })
        .collect::<TestResult<Vec<_>>>()?;
    assert_eq!(
        targets,
        [
            common::guid(fixture::CATEGORY),
            common::guid(fixture::LOWER)
        ],
        "the excluded database has real authored data"
    );
    Ok(())
}

#[test]
fn category_v3_distinguishes_declared_empty_absent_and_fully_filtered_families() -> TestResult {
    let declared = fixture::schema();
    let absent = DataForgeBuilder::new().version(8);
    let filtered = fixture::single();
    fixture::original(
        &filtered,
        0,
        fixture::CATEGORY,
        fixture::RAW_NAME,
        fixture::FILE_NAME,
        fixture::TAG,
    )?;
    let filter = Filter::new("*/outside/*");
    for (source, selected, availability) in [
        (&declared, None, "available"),
        (&absent, None, "absent"),
        (&filtered, Some(&filter), "available"),
    ] {
        let bytes = source.build();
        let forge = DataForge::open(&bytes)?;
        let names = forge
            .structs()
            .map(|entry| entry.name())
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            names.contains(&fixture::SOURCE_TYPE),
            availability == "available",
            "availability follows the actual source declaration"
        );
        let (built, catalog) = fixture::build(
            source,
            DcbOrigin::Plain,
            CatalogPublicationMode::Normal,
            selected,
        )?;
        fixture::counts(&built, 0, 0)?;
        assert!(
            fixture::rows(&catalog)?.is_empty(),
            "these source scopes produce no selected rows"
        );
        assert_eq!(
            fixture::at(&catalog, "/definitions/blueprintCategories/availability")?,
            &json!(availability),
            "empty rows do not erase source-family availability"
        );
    }
    Ok(())
}
