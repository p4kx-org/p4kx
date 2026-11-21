// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The published global Tag count and its localized filtered-catalog summary.

use std::fs;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_export::{CatalogSummary, Filter, Kind};
use p4kx_gui::{CatalogExportOptions, CatalogExportStatus, Language, Model, ui};
use serde_json::{Value, json};

use crate::cataloging::{self, TestResult, opened};
use crate::waiting::{RunQuiet, wait_for};

#[test]
fn filtered_catalogs_show_the_published_global_tag_count_in_both_languages() -> TestResult {
    for (language, tags, manufacturers) in [
        (
            Language::BritishEnglish,
            "Global tags: 2 written, 0 skipped, complete",
            "Manufacturers: 1 written, 0 skipped, partial",
        ),
        (
            Language::German,
            "Globale Tags: 2 geschrieben, 0 übersprungen, vollständig",
            "Hersteller: 1 geschrieben, 0 übersprungen, teilweise",
        ),
    ] {
        check_language(language, tags, manufacturers)?;
    }
    Ok(())
}

fn check_language(language: Language, tags: &str, manufacturers: &str) -> TestResult {
    let (_directory, mut model, out) = opened(&cataloging::tag_taxonomy())?;
    model.set_language(language);
    let file = out.join("filtered-tags.json");
    assert!(
        model.start_catalog_with_options(
            file.clone(),
            CatalogExportOptions {
                filter: Some(Filter::new("included")),
                ..CatalogExportOptions::default()
            },
            || {},
        ),
        "the filtered normal catalog starts"
    );
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1400.0, 900.0])
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            model,
        );
    wait_for(
        &mut harness,
        "the filtered Tag catalog to finish",
        |harness| {
            harness
                .state()
                .catalog()
                .is_some_and(|run| !run.is_running())
        },
    )?;
    harness.run_quiet();
    let run = harness.state().catalog().ok_or("no catalog job")?;
    let CatalogExportStatus::Finished(outcome) = run.status() else {
        return Err("the filtered normal catalog must publish successfully".into());
    };
    assert_summary(outcome.summary())?;
    let catalog: Value = serde_json::from_slice(&fs::read(file)?)?;
    assert_published(&catalog);
    harness.get_by_label(manufacturers).scroll_to_me();
    harness.run_quiet();
    harness.get_by_label(manufacturers);
    harness.get_by_label(tags).scroll_to_me();
    harness.run_quiet();
    harness.get_by_label(tags);
    Ok(())
}

fn assert_summary(summary: &CatalogSummary) -> TestResult {
    let tags = summary.kind(Kind::Tags).ok_or("no Tag summary")?;
    assert_eq!(
        (tags.count, tags.skipped, tags.complete),
        (2, 0, true),
        "both physical Tags remain global, including the Test_ source"
    );
    let manufacturers = summary
        .kind(Kind::Manufacturers)
        .ok_or("no manufacturer summary")?;
    assert_eq!(
        (
            manufacturers.count,
            manufacturers.skipped,
            manufacturers.complete
        ),
        (1, 0, false),
        "the same job retains the independent manufacturer Filter scope"
    );
    assert!(
        summary.export_health().is_complete(),
        "a partial selection can have complete source and projection health"
    );
    Ok(())
}

fn assert_published(catalog: &Value) {
    assert_eq!(catalog["schemaVersion"], 3, "the GUI publishes only v3");
    for (path, count) in [
        ("/definitions/tags/rows", 2),
        ("/declarations/tagRoots/rows", 1),
        ("/projections/tagPaths/rows", 2),
        ("/definitions/manufacturers/rows", 1),
    ] {
        assert_eq!(
            catalog
                .pointer(path)
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(count),
            "the committed {path} has its own accounting unit"
        );
    }
    assert_eq!(
        catalog.pointer("/completeness/tags"),
        Some(&json!({"status": "complete"})),
        "the published taxonomy scope is global"
    );
    assert_eq!(
        catalog.pointer("/completeness/manufacturers"),
        Some(&json!({"status": "partial", "filter": "included"})),
        "the published owner scope retains its actual Filter"
    );
    assert!(catalog.get("tags").is_none(), "no duplicate flat Tag view");
    assert_eq!(
        catalog
            .pointer("/exportHealth/publicationMode")
            .and_then(Value::as_str),
        Some("normal"),
        "the adapter publishes without diagnostic permission"
    );
    assert_eq!(
        catalog
            .pointer("/exportHealth/status")
            .and_then(Value::as_str),
        Some("complete"),
        "the published source and projection health remain complete"
    );
}
