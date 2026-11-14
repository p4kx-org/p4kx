// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::CatalogPublicationMode;
use serde_json::{Value, json};

use crate::blueprint_category_v3_fixture as fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn category_v3_emits_only_the_canonical_source_header_without_invented_text() -> TestResult {
    let source = fixture::single();
    fixture::original(
        &source,
        0,
        fixture::CATEGORY,
        fixture::RAW_NAME,
        fixture::FILE_NAME,
        fixture::TAG,
    )?;
    fixture::fieldless(&source, 0)?;
    let (built, catalog) = fixture::build(
        &source,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        None,
    )?;
    fixture::counts(&built, 1, 0)?;
    for domain in ["publicNames", "gameTexts"] {
        assert_eq!(
            fixture::at(&catalog, &format!("/texts/{domain}"))?,
            &json!([]),
            "a fieldless category supplies no authored text key"
        );
    }
    assert_definition(&catalog)?;
    Ok(())
}

#[test]
fn category_v3_keeps_duplicate_physical_guids_and_original_equal_guid_order() -> TestResult {
    let first = "libs/foundry/records/crafting/blueprintcategories/First.XML";
    let second = "libs/foundry/records/crafting/blueprintcategories/Second.XML";
    let lower = "libs/foundry/records/crafting/blueprintcategories/Lower.XML";
    let source = fixture::schema()
        .record(
            fixture::category(fixture::CATEGORY, "BlueprintCategoryRecord.First", first)
                .tag("FirstDomain"),
        )
        .record(
            fixture::category(fixture::CATEGORY, "BlueprintCategoryRecord.Second", second)
                .tag("SecondDomain"),
        )
        .record(
            fixture::category(fixture::LOWER, "BlueprintCategoryRecord.Lower", lower)
                .tag("LowerDomain"),
        );
    for (index, guid, name, file, tag) in [
        (
            0,
            fixture::CATEGORY,
            "BlueprintCategoryRecord.First",
            first,
            "FirstDomain",
        ),
        (
            1,
            fixture::CATEGORY,
            "BlueprintCategoryRecord.Second",
            second,
            "SecondDomain",
        ),
        (
            2,
            fixture::LOWER,
            "BlueprintCategoryRecord.Lower",
            lower,
            "LowerDomain",
        ),
    ] {
        fixture::original(&source, index, guid, name, file, tag)?;
        fixture::fieldless(&source, index)?;
    }
    let (built, catalog) = fixture::diagnostic(&source)?;
    fixture::counts(&built, 3, 0)?;
    assert_duplicate_rows(&catalog, first, second, lower)?;
    Ok(())
}

fn assert_definition(catalog: &Value) -> TestResult {
    let row = fixture::row(catalog, 0)?;
    assert_eq!(
        row.as_object()
            .ok_or("category object")?
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(fixture::FIELDS),
        "the complete fieldless definition has exactly the eight public header cells"
    );
    assert_eq!(
        fixture::at(catalog, "/schemaVersion")?,
        &json!(3),
        "catalog v3 only"
    );
    assert!(
        catalog.get("blueprintCategories").is_none(),
        "the superseded flat root list is absent"
    );
    assert_eq!(
        fixture::at(catalog, "/definitions/blueprintCategories/availability")?,
        &json!("available"),
        "the source family is declared"
    );
    for (field, expected) in [
        ("guid", json!(common::guid(fixture::CATEGORY))),
        ("recordName", json!(fixture::RAW_NAME)),
        ("className", json!("RawCategory")),
        ("recordPath", json!(fixture::PATH)),
        ("recordIndex", json!(0)),
        ("sourceType", json!(fixture::SOURCE_TYPE)),
        ("fileName", json!(fixture::FILE_NAME)),
        ("metadataTag", json!(fixture::TAG)),
    ] {
        assert_eq!(
            fixture::at(row, &format!("/{field}/state"))?,
            &json!("present"),
            "the header fact remains present"
        );
        assert_eq!(
            fixture::at(row, &format!("/{field}/value"))?,
            &expected,
            "the actual header value survives"
        );
    }
    Ok(())
}

fn assert_duplicate_rows(catalog: &Value, first: &str, second: &str, lower: &str) -> TestResult {
    let rows = fixture::rows(catalog)?;
    let indices = rows
        .iter()
        .map(|row| {
            fixture::at(row, "/recordIndex/value")?
                .as_u64()
                .ok_or_else(|| "original physical row index".into())
        })
        .collect::<TestResult<Vec<_>>>()?;
    assert_eq!(
        indices,
        [2, 0, 1],
        "GUID order preserves the original order of equal-GUID physical rows"
    );
    for (index, guid, file, tag) in [
        (0, fixture::CATEGORY, first, "FirstDomain"),
        (1, fixture::CATEGORY, second, "SecondDomain"),
        (2, fixture::LOWER, lower, "LowerDomain"),
    ] {
        let row = fixture::row(catalog, index)?;
        assert_eq!(
            fixture::at(row, "/guid/value")?,
            &json!(common::guid(guid)),
            "nominal GUID is retained"
        );
        assert_eq!(
            fixture::at(row, "/fileName/value")?,
            &json!(file),
            "physical filename is not borrowed from a same-GUID alias"
        );
        assert_eq!(
            fixture::at(row, "/metadataTag/value")?,
            &json!(tag),
            "each physical tag stays with its own row"
        );
    }
    Ok(())
}
