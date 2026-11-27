// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_extraction::{Archive, Guid, OpenOptions, SourceInventory, SourceLocation};
use p4kx_testkit::{TestElement, TestEntry};
use serde_json::{Value, json};

use super::{external_at, open, root, socpak, system};
use crate::catalog_read::original::with_entry_capture;
use crate::starmap_containers::{PlacementScope, Placements};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn fixture() -> Vec<TestEntry> {
    let invalid = TestElement::new("Child")
        .attribute("name", "C:/authoring/unsearchable.socpak")
        .attribute("external", "true")
        .attribute("pos", "4,5,6")
        .attribute("starMapRecord", "not a GUID");
    let children = vec![
        external_at("missing", "1,2,3").attribute("starMapRecord", Guid::default().to_string()),
        external_at("missing", "2,3,4"),
        invalid,
        external_at("ambiguous", "3,4,5"),
    ];
    let mut entries = system(&root(children), &[]);
    let empty = root(Vec::new());
    entries.push(socpak("ambiguous", &empty));
    entries.push(socpak("ambiguous", &empty));
    entries
}

#[test]
fn unresolved_child_resolution_preserves_slots_and_directory_evidence() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(dir.path(), fixture());
    let (rows, inventory) = read_rows(&archive)?;
    assert_eq!(rows.len(), 4, "each original external Child remains");
    assert_eq!(
        rows[0]["declaration"]["starmapRecordGuid"]["value"],
        Guid::default().to_string(),
        "an authored zero GUID remains a present original datum"
    );
    assert_eq!(
        rows[1]["declaration"]["starmapRecordGuid"]["state"], "absent",
        "a missing original attribute stays absent"
    );
    assert_eq!(
        rows[2]["declaration"]["starmapRecordGuid"]["state"], "invalid",
        "malformed original GUID text is distinct from zero"
    );
    let states: Vec<_> = rows
        .iter()
        .map(|row| &row["targetStatus"]["value"])
        .collect();
    assert_eq!(
        states,
        vec![
            &json!("notFound"),
            &json!("notFound"),
            &json!("unsupportedSelector"),
            &json!("ambiguous")
        ]
    );
    assert_ne!(
        rows[0]["instanceId"], rows[1]["instanceId"],
        "separate authored equal target slots"
    );
    assert_ne!(
        rows[0]["declaration"]["declaredPath"], rows[1]["declaration"]["declaredPath"],
        "separate original path origins"
    );
    assert_eq!(
        rows[3]["targetCandidates"]["value"]
            .as_array()
            .ok_or("ambiguous candidates")?
            .len(),
        2
    );
    assert_ne!(
        rows[3]["targetCandidates"]["value"][0]["value"],
        rows[3]["targetCandidates"]["value"][1]["value"],
        "actual distinct physical candidate identity"
    );
    for row in &rows {
        verify_directory_origin(row, &inventory)?;
    }
    assert_eq!(rows[0]["targetCandidates"]["sourceSetComplete"], true);
    assert_eq!(
        rows[2]["targetCandidates"]["sourceSetComplete"], false,
        "unsupported grammar cannot prove a complete lookup"
    );
    Ok(())
}

#[test]
fn recovered_directory_does_not_prove_an_absent_external_target() -> TestResult {
    const END: &[u8; 4] = b"PK\x05\x06";
    let dir = tempfile::tempdir()?;
    let archive = open(dir.path(), fixture());
    assert!(
        crate::export_archive_health::inventory(&archive).is_none(),
        "intact source directory"
    );
    drop(archive);
    let path = dir.path().join("Data.p4k");
    let mut bytes = fs::read(&path)?;
    let end = bytes
        .windows(END.len())
        .rposition(|part| part == END)
        .ok_or("end record")?;
    bytes.truncate(end);
    fs::write(&path, bytes)?;
    let archive = Archive::open_with(&path, OpenOptions::new().recover(true))?;
    assert!(
        crate::export_archive_health::inventory(&archive).is_some(),
        "real directory recovery"
    );
    let (rows, inventory) = read_rows(&archive)?;
    assert_eq!(rows.len(), 4);
    for row in &rows[..3] {
        assert_eq!(
            row["targetStatus"]["value"], "incompleteDirectory",
            "root membership is uncertain"
        );
        assert_eq!(
            row["targetCandidates"]["sourceSetComplete"], false,
            "no invented complete absence"
        );
        verify_directory_origin(row, &inventory)?;
    }
    assert_eq!(
        rows[3]["targetStatus"]["value"], "ambiguous",
        "actual duplicate candidates remain identified"
    );
    assert_eq!(rows[3]["targetCandidates"]["sourceSetComplete"], false);
    Ok(())
}

fn read_rows(archive: &Archive) -> TestResult<(Vec<Value>, SourceInventory)> {
    with_entry_capture(archive, |capture| {
        let mut placements =
            Placements::read(archive, None, capture, PlacementScope::RecordSelection)?;
        let encoded = serde_json::to_value(placements.take_unresolved())
            .map_err(crate::CatalogError::Json)?;
        let failed = crate::export_health_entries::compile(placements.ledger.entries)?;
        assert!(
            !failed.failed.is_empty(),
            "actual source failures retain the existing gate evidence"
        );
        let rows = encoded["rows"]
            .as_array()
            .ok_or(crate::ExportAccountingError::ConflictingSourceDatum)?
            .clone();
        Ok((rows, capture.source_uses().freeze()?))
    })
    .map_err(Into::into)
}

fn verify_directory_origin(row: &Value, inventory: &SourceInventory) -> TestResult {
    assert_eq!(
        row["targetStatus"]["sourceUsesRefs"], row["targetCandidates"]["sourceUsesRefs"],
        "lookup result cites the original selector and directory survey"
    );
    assert_eq!(
        row["targetStatus"]["sourceSetComplete"], row["targetCandidates"]["sourceSetComplete"],
        "lookup status retains the proven directory scope"
    );
    let refs = row["targetCandidates"]["sourceUsesRefs"]
        .as_array()
        .ok_or("candidate source-use references")?;
    let directory = inventory
        .uses()
        .iter()
        .filter(|usage| {
            matches!(
                usage.location(),
                SourceLocation::ArchiveDirectoryPaths { .. }
            )
        })
        .collect::<Vec<_>>();
    assert!(
        !directory.is_empty(),
        "original complete consulted root directory path range"
    );
    assert!(
        directory
            .iter()
            .any(|usage| refs.contains(&json!(usage.id().as_str()))),
        "target membership cites original directory survey"
    );
    Ok(())
}
