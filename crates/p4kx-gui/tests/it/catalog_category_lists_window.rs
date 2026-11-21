// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! An ordinary filtered category-list export through the shared headless GUI window.

use std::error::Error;
use std::fs;
use std::time::UNIX_EPOCH;

use p4kx_export::{
    self as export, CatalogOptions, CatalogSummary, ExportSourceCounts, ExportStageStatus, Filter,
    Kind,
};
use p4kx_extraction::{Archive, DataForge, Value as Raw};
use p4kx_gui::{CatalogExportOptions, CatalogExportStatus, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

use crate::cataloging::{self, TestResult};

const LIST_TYPE: &str = "BlueprintCategoryDatabaseRecord";
const CATEGORY_TYPE: &str = "BlueprintCategoryRecord";
const FILTER: &str = "synthetic/included";
const INCLUDED: &str = "synthetic/included/Primary.xml";
const EXCLUDED: &str = "synthetic/excluded/Secondary.xml";
const TARGET: &str = "libs/foundry/records/crafting/blueprintcategories/Target.xml";
const TARGET_GUID: &str = "00000000-0000-0000-0000-000000000303";

#[test]
fn category_lists_keep_normal_filter_health_and_both_visible_labels() -> TestResult {
    let database = database();
    assert_original_source(&database)?;
    let archive =
        ArchiveBuilder::new().entry(TestEntry::stored("Data\\Game2.dcb", database.build()));
    assert_public_build(&archive)?;
    for (language, label) in [
        (
            Language::BritishEnglish,
            "Blueprint category lists: 1 written, 0 skipped, partial",
        ),
        (
            Language::German,
            "Blaupausenkategorielisten: 1 geschrieben, 0 \u{fc}bersprungen, teilweise",
        ),
    ] {
        check_window(&archive, language, label)?;
    }
    Ok(())
}

fn database() -> DataForgeBuilder {
    let mut database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Padding"))
        .structure(TestStruct::new(CATEGORY_TYPE))
        .structure(TestStruct::new(LIST_TYPE).array(
            "categories",
            TestType::ReferenceTo(CATEGORY_TYPE.to_owned()),
        ));
    for (guid, name, path) in [(0x301, "Primary", INCLUDED), (0x302, "Secondary", EXCLUDED)] {
        database = database.record(
            TestRecord::new(guid, LIST_TYPE, format!("{LIST_TYPE}.{name}"), path).value(
                "categories",
                TestValue::Array(vec![TestValue::Reference(0x303)]),
            ),
        );
    }
    database.record(TestRecord::new(
        0x303,
        CATEGORY_TYPE,
        "BlueprintCategoryRecord.Target",
        TARGET,
    ))
}

fn assert_original_source(database: &DataForgeBuilder) -> TestResult {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    assert_eq!(
        forge
            .records()
            .filter(|record| record.type_name() == Ok(LIST_TYPE))
            .count(),
        2,
        "the actual source has two physical database owners"
    );
    let filter = Filter::new(FILTER);
    let target_path = TARGET
        .strip_suffix(".xml")
        .ok_or("fixture target extension")?;
    for (index, path, selected) in [(0, INCLUDED, true), (1, EXCLUDED, false)] {
        let record = forge.record(index).ok_or("original database owner")?;
        assert_eq!(
            record.stored_file_name()?,
            path,
            "original physical owner path"
        );
        assert_eq!(
            filter.matches(record.record_path()?),
            selected,
            "ordinary owner selection"
        );
        let tree = record.tree()?;
        let Some(Raw::Array(slots)) = tree.root().field("categories") else {
            return Err("original categories array".into());
        };
        assert_eq!(slots.len(), 1, "one original slot per owner");
        let Some(Raw::Reference(target)) = slots.first() else {
            return Err("original resolved category reference".into());
        };
        assert_eq!(
            target.record_index(),
            2,
            "the original target is outside both list owners"
        );
        assert_eq!(
            target.guid().to_string(),
            TARGET_GUID,
            "original authored target GUID"
        );
        assert_eq!(
            target.type_name(),
            CATEGORY_TYPE,
            "original declared target type"
        );
        assert_eq!(target.record_path(), target_path, "original target path");
    }
    assert!(
        !filter.matches(TARGET),
        "the original target lies outside the selected scope"
    );
    Ok(())
}

fn assert_public_build(builder: &ArchiveBuilder) -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    builder.write_to(&path)?;
    let archive = Archive::open(path)?;
    let filter = Filter::new(FILTER);
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.filter = Some(&filter);
    let built = export::build_catalog(&archive, &options)?;
    assert_summary(built.summary())?;
    assert_published(&serde_json::from_slice(built.json())?)
}

fn check_window(builder: &ArchiveBuilder, language: Language, label: &str) -> TestResult {
    use crate::waiting::RunQuiet;
    use egui_kittest::kittest::Queryable;

    let (_directory, mut model, out) = cataloging::opened(builder)?;
    model.set_language(language);
    let file = out.join("filtered-category-lists.json");
    assert!(
        model.start_catalog_with_options(
            file.clone(),
            CatalogExportOptions {
                filter: Some(Filter::new(FILTER)),
                ..Default::default()
            },
            || {},
        ),
        "the real normal GUI catalog job starts"
    );
    let mut harness = super::harness(model);
    super::wait_for_the_end(&mut harness)?;
    let run = harness.state().model.catalog().ok_or("catalog job")?;
    let CatalogExportStatus::Finished(outcome) = run.status() else {
        return Err("the normal filtered catalog must publish".into());
    };
    assert_summary(outcome.summary())?;
    assert_published(&serde_json::from_slice(&fs::read(file)?)?)?;
    harness.get_by_label(label).scroll_to_me();
    harness.run_quiet();
    harness.get_by_label(label);
    Ok(())
}

fn assert_summary(summary: &CatalogSummary) -> TestResult {
    let kind = summary
        .kind(Kind::BlueprintCategoryLists)
        .ok_or("category-list summary")?;
    assert_eq!(
        (kind.count, kind.skipped, kind.complete),
        (1, 0, false),
        "one selected physical list retains ordinary partial selection scope"
    );
    assert!(
        summary.export_health().is_complete(),
        "the selected source is fully readable"
    );
    let health = summary
        .export_health()
        .kind(Kind::BlueprintCategoryLists)
        .ok_or("category-list source health")?;
    assert!(
        health.source_inventory_complete,
        "the selected source inventory is complete"
    );
    assert!(
        health.field_inventory_complete,
        "all selected source fields were inspected"
    );
    assert_eq!(
        (health.read, health.projection),
        (ExportStageStatus::Complete, ExportStageStatus::Complete),
        "partial scope is independent from complete selected read/projection"
    );
    assert_eq!(
        health.source_records,
        ExportSourceCounts {
            selected: 1,
            read_successful: 1,
            projected: 1,
            ..Default::default()
        },
        "the excluded owner and target do not become selected records or dependencies"
    );
    assert_eq!(
        health.written_rows, 1,
        "the summary counts the actual published row"
    );
    Ok(())
}

fn assert_published(catalog: &Value) -> TestResult {
    for (path, expected) in [
        ("/schemaVersion", json!(3)),
        ("/exportHealth/publicationMode", json!("normal")),
        ("/exportHealth/status", json!("complete")),
        (
            "/completeness/blueprintCategoryLists",
            json!({"status":"partial","filter":FILTER}),
        ),
        (
            "/declarations/blueprintCategoryLists/availability",
            json!("available"),
        ),
    ] {
        assert_eq!(at(catalog, path)?, &expected, "actual committed {path}");
    }
    let rows = at(catalog, "/declarations/blueprintCategoryLists/rows")?
        .as_array()
        .ok_or("published category-list rows")?;
    assert_eq!(
        rows.len(),
        1,
        "the GUI file contains exactly the selected physical owner"
    );
    let row = rows.first().ok_or("selected list row")?;
    for (path, expected) in [
        ("/recordIndex/value", json!(0)),
        ("/sourceType/value", json!(LIST_TYPE)),
        ("/fileName/value", json!(INCLUDED)),
        ("/categories/value/0/value/status", json!("present")),
        ("/categories/value/0/value/guid/value", json!(TARGET_GUID)),
        (
            "/categories/value/0/value/targetRecordIndex/value",
            json!(2),
        ),
        (
            "/categories/value/0/value/targetType/value",
            json!(CATEGORY_TYPE),
        ),
    ] {
        assert_eq!(
            at(row, path)?,
            &expected,
            "the selected original list preserves {path}"
        );
    }
    assert_eq!(
        at(row, "/categories/value")?.as_array().map(Vec::len),
        Some(1),
        "the original slot count is preserved"
    );
    assert_eq!(
        at(catalog, "/definitions/blueprintCategories/rows")?
            .as_array()
            .map(Vec::len),
        Some(0),
        "resolving a reference does not backfill the excluded target category"
    );
    assert_source_uses(catalog, row)
}

fn assert_source_uses(catalog: &Value, row: &Value) -> TestResult {
    assert_use(
        catalog,
        at(row, "/categories/value/0/value/guid")?,
        &json!({"type":"dataForge","recordIndex":0,"field":"categories[0]"}),
        "payload",
    )?;
    assert_use(
        catalog,
        at(row, "/categories/value/0/value/targetRecordIndex")?,
        &json!({"type":"subresource","namespace":"dataforgeRecordHeader","indices":["2"],"field":"recordIndex"}),
        "selector",
    )
}

fn assert_use(catalog: &Value, carrier: &Value, location: &Value, role: &str) -> TestResult {
    let references = at(carrier, "/sourceUsesRefs")?
        .as_array()
        .ok_or("immediate source uses")?;
    assert_eq!(
        references.len(),
        1,
        "this datum has only its actual source contribution"
    );
    let id = references.first().ok_or("source-use reference")?;
    let inventory = at(catalog, "/provenance/uses")?
        .as_array()
        .ok_or("source-use inventory")?;
    let mut matches = inventory.iter().filter(|used| used.get("id") == Some(id));
    let used = matches.next().ok_or("closed source-use reference")?;
    assert!(
        matches.next().is_none(),
        "the source-use id resolves uniquely"
    );
    assert_eq!(
        at(used, "/location")?,
        location,
        "original body and target header remain distinct"
    );
    assert_eq!(
        at(used, "/role")?,
        &json!(role),
        "only the actual contributing role is retained"
    );
    assert_eq!(
        at(used, "/scope")?,
        &json!("payload"),
        "the source byte domain stays explicit"
    );
    let sources = at(catalog, "/provenance/sources")?
        .as_array()
        .ok_or("physical input inventory")?;
    let source = sources
        .iter()
        .find(|source| source.get("id") == used.get("source"))
        .ok_or("closed original DCB input")?;
    assert_eq!(
        at(source, "/kind")?,
        &json!("archiveEntry"),
        "an actual Archive entry owns this fact"
    );
    assert_eq!(
        at(source, "/path/value")?,
        &json!("Data\\Game2.dcb"),
        "the original DCB owns this fact"
    );
    Ok(())
}

fn at<'a>(value: &'a Value, path: &str) -> Result<&'a Value, Box<dyn Error>> {
    value
        .pointer(path)
        .ok_or_else(|| format!("missing {path}").into())
}
