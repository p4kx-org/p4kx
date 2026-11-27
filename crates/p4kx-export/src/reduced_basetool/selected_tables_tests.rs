// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::ReducedError;
use super::plan;
use super::selected_tables::SelectedTables;
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_table::DatasetAvailability;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const METADATA_AND_CONTAINERS: &[&str] = &[
    "completeness",
    "exportHealth",
    "schemaVersion",
    "shipStorePledges",
    "source",
    "texts",
    "configurations",
    "declarations",
    "definitions",
    "placements",
    "projections",
];

#[test]
fn selected_tables_publish_only_complete_fixed_selection_and_actual_row_counts() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let mut staged = SelectedTables::default();
    for path in plan::SELECTION {
        if METADATA_AND_CONTAINERS.contains(&path) {
            continue;
        }
        if path == "definitions.blueprints" {
            let mut rows = StagedCatalogRows::new(scratch.path(), None);
            rows.append(&json!({"recordIndex": 16}))?;
            rows.append(&json!({"recordIndex": 9}))?;
            staged.table(path, rows.finish(DatasetAvailability::Available)?)?;
        } else if path.contains('.') {
            staged.table(path, StagedCatalogTable::empty(DatasetAvailability::Absent))?;
        } else {
            staged.sequence(
                path,
                StagedCatalogRows::new(scratch.path(), None).finish_sequence()?,
            )?;
        }
    }
    let tables = staged.finish()?;
    let wire = serde_json::to_value(&tables)?;
    assert!(tables.take_spool_failure().is_none());
    assert_eq!(
        wire.pointer("/definitions/blueprints/rows"),
        Some(&json!([{"recordIndex": 16}, {"recordIndex": 9}])),
        "serialization keeps spooled row order"
    );
    let coverage = wire
        .pointer("/completeness")
        .and_then(Value::as_array)
        .ok_or("complete coverage")?;
    assert_eq!(
        coverage.len(),
        plan::SELECTION.len() - METADATA_AND_CONTAINERS.len()
    );
    for (path, selection, count) in [
        ("definitions.blueprints", "all", 2),
        ("loadouts", "shipAndVehicleRoots", 0),
        ("placements.starmapPlacements", "boundStarmapObjects", 0),
    ] {
        assert!(
            coverage.contains(&json!({"path":path, "selection":selection, "rows":count})),
            "{path} uses its observed row count and declared selection"
        );
    }
    Ok(())
}

#[test]
fn selected_tables_refuse_misplaced_duplicate_excluded_or_missing_payloads() -> TestResult {
    let mut staged = SelectedTables::default();
    let table = || StagedCatalogTable::empty(DatasetAvailability::Absent);
    for path in [
        "definitions",
        "texts.gameTexts",
        "shops",
        "definitions.unselected",
    ] {
        assert!(
            matches!(staged.table(path, table()), Err(ReducedError::IncompleteScope(actual)) if path == actual),
            "the fixed wire schema rejects table {path}"
        );
    }
    let valid = "definitions.blueprints";
    assert!(
        matches!(
            staged.table(valid, StagedCatalogTable::empty(DatasetAvailability::Unavailable)),
            Err(ReducedError::IncompleteScope(actual)) if valid == actual
        ),
        "unreadable source declarations cannot be reported as absent"
    );
    staged.table(valid, table())?;
    assert!(
        matches!(staged.table(valid, table()), Err(ReducedError::IncompleteScope(actual)) if valid == actual)
    );
    assert!(
        matches!(
            staged.finish(),
            Err(ReducedError::IncompleteScope(
                "configurations.attachableModifiers"
            ))
        ),
        "the first uncovered leaf blocks composition"
    );
    Ok(())
}
