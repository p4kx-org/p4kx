// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter};
use p4kx_extraction::DataForge;
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::blueprint_category_v3_fixture;
use crate::category_lists_v3_fixture as fixture;
use crate::category_lists_v3_source as source;
use crate::commodity_v3_fixture::{DcbOrigin, TestResult};

#[test]
fn category_lists_v3_filters_owners_without_backfilling_or_erasing_physical_targets() -> TestResult
{
    let database = fixture::clean();
    source::ordered(&database, false)?;
    assert_database_filter(&database)?;
    assert_category_filter(&database)
}

fn assert_database_filter(database: &DataForgeBuilder) -> TestResult {
    let databases = Filter::new("synthetic/category_lists/*");
    let (built, catalog) = fixture::build(
        database,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        Some(&databases),
    )?;
    fixture::count(&built, 1, 1)?;
    assert!(
        blueprint_category_v3_fixture::rows(&catalog)?.is_empty(),
        "a DB-only Filter cannot auto-include category definitions"
    );
    let row = fixture::row(&catalog, 0)?;
    assert_eq!(
        fixture::at(row, "/categories/value/0/value/status")?,
        &json!("present"),
        "catalog omission does not erase source resolution"
    );
    assert_eq!(
        fixture::at(row, "/categories/value/0/value/targetRecordIndex/value")?,
        &json!(3),
        "the exact excluded physical target stays known"
    );
    assert_scope(&catalog, "partial")?;
    Ok(())
}

fn assert_category_filter(database: &DataForgeBuilder) -> TestResult {
    let categories = Filter::new("*/blueprintcategories/*");
    let (built, catalog) = fixture::build(
        database,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        Some(&categories),
    )?;
    fixture::count(&built, 0, 0)?;
    assert!(
        fixture::rows(&catalog)?.is_empty(),
        "category-only selection cannot synthesize a list owner"
    );
    assert_eq!(
        blueprint_category_v3_fixture::rows(&catalog)?.len(),
        2,
        "the independent category Kind keeps its existing scope"
    );
    assert_scope(&catalog, "partial")?;
    Ok(())
}

fn assert_scope(catalog: &Value, expected: &str) -> TestResult {
    assert_eq!(
        fixture::at(catalog, "/completeness/blueprintCategoryLists/status")?,
        &json!(expected),
        "lists retain ordinary Filter completeness"
    );
    assert_eq!(
        fixture::at(catalog, "/completeness/tags/status")?,
        &json!("complete"),
        "the existing global taxonomy exemption is not broadened to lists"
    );
    assert_eq!(
        fixture::at(catalog, "/declarations/blueprintCategoryLists/availability")?,
        &json!("available"),
        "filtering owners does not erase the source declaration"
    );
    assert_eq!(
        fixture::at(catalog, "/exportHealth/status")?,
        &json!("complete"),
        "a supported partial selection does not become a read failure"
    );
    Ok(())
}

#[test]
fn category_lists_v3_distinguishes_declared_empty_absent_and_fully_filtered_sources() -> TestResult
{
    let declared = fixture::schema();
    let absent = DataForgeBuilder::new().version(8);
    let populated = fixture::clean();
    source::ordered(&populated, false)?;
    let filter = Filter::new("*/nowhere/*");
    for (database, selected, available) in [
        (&declared, None, "available"),
        (&absent, None, "absent"),
        (&populated, Some(&filter), "available"),
    ] {
        check_availability(database, selected, available)?;
    }
    Ok(())
}

fn check_availability(
    database: &DataForgeBuilder,
    selected: Option<&Filter>,
    available: &str,
) -> TestResult {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let names = forge
        .structs()
        .map(|entry| entry.name())
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        names.contains(&fixture::TYPE),
        available == "available",
        "availability follows the original declared family"
    );
    let (built, catalog) = fixture::build(
        database,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        selected,
    )?;
    fixture::count(&built, 0, 0)?;
    assert!(
        fixture::rows(&catalog)?.is_empty(),
        "no selected owners means no authored lists"
    );
    assert_eq!(
        fixture::at(
            &catalog,
            "/declarations/blueprintCategoryLists/availability"
        )?,
        &json!(available),
        "empty selection and absent family remain distinct"
    );
    let completeness = if selected.is_some() {
        "partial"
    } else {
        "complete"
    };
    assert_eq!(
        fixture::at(&catalog, "/completeness/blueprintCategoryLists/status")?,
        &json!(completeness),
        "source availability and Filter scope are independent"
    );
    Ok(())
}
