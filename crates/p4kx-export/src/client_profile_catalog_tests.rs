// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_extraction::Filter;
use p4kx_testkit::{DataForgeBuilder, TestEntry, TestRecord, TestStruct, test_key};
use serde_json::{Value, json};

use super::{GENERAL, GENERAL_SOURCE, GENERAL_XML, TestResult, VR, archive, at, rows};
use crate::{CatalogError, CatalogOptions, Kind, build_catalog};

#[test]
fn catalog_selects_profile_documents_and_counts_only_read_physical_entries() -> TestResult {
    let (_directory, archive) = archive(
        vec![
            dataforge(),
            TestEntry::stored(GENERAL, GENERAL_XML),
            TestEntry::stored(
                VR,
                "<Attributes><Attr name='fixture' value='1'/></Attributes>",
            )
            .encrypted(),
        ],
        Some(test_key()),
    )?;
    let filter = Filter::parse(VR)?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.filter = Some(&filter);
    let result = build_catalog(&archive, options.with_full_provenance())?;
    let document: Value = serde_json::from_slice(result.json())?;
    assert_eq!(
        at(&document, "/completeness/clientProfileSettings")?,
        &json!({"status": "partial", "filter": VR}),
        "a profile Archive Entry filter must be visible in selection coverage"
    );
    let profiles = at(&document, "/configurations/clientProfileSettings/documents")?;
    assert_eq!(
        at(profiles, "/0/status/value")?,
        "filtered",
        "the general XML is not selected"
    );
    assert_eq!(
        at(profiles, "/1/status/value")?,
        "read",
        "the selected VR source is decoded"
    );
    assert_eq!(
        at(
            profiles,
            "/1/elements/1/declarations/1/value/value/containsEncryptedSource"
        )?,
        true,
        "the Catalog shares the parser's physical Archive evidence"
    );
    let health = rows(&document, "/exportHealth/kinds")?
        .iter()
        .find(|row| row.get("kind") == Some(&json!("clientProfileSettings")))
        .ok_or("the selected profile kind has no export health row")?;
    assert_eq!(at(health, "/sourceMode")?, "archiveEntries");
    assert_eq!(at(health, "/writtenRows")?, 1);
    assert_eq!(at(health, "/sourceEntries/projected")?, 1);
    Ok(())
}

#[test]
fn malformed_selected_profile_refuses_normal_catalog_publication() -> TestResult {
    let (_directory, archive) = archive(
        vec![dataforge(), TestEntry::stored(VR, "<Attributes>")],
        None,
    )?;
    let filter = Filter::parse(VR)?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.filter = Some(&filter);
    assert!(
        matches!(
            build_catalog(&archive, options.with_full_provenance()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "the source projection failure cannot publish a complete Catalog"
    );
    Ok(())
}

#[test]
fn ambiguous_profile_candidates_are_selection_failures_without_read_counts() -> TestResult {
    let (_directory, archive) = archive(
        vec![
            dataforge(),
            TestEntry::stored(GENERAL, GENERAL_XML),
            TestEntry::stored(GENERAL_SOURCE, GENERAL_XML),
            TestEntry::stored(VR, GENERAL_XML),
        ],
        None,
    )?;
    let result = build_catalog(
        &archive,
        CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
    );
    let Err(CatalogError::IncompleteExport(refusal)) = result else {
        return Err("selection ambiguity must refuse Catalog publication".into());
    };
    let health = refusal.health();
    let kind = health
        .kind(Kind::ClientProfileSettings)
        .ok_or("the profile export must report its separate Archive health")?;
    assert_eq!(kind.source_entries.selected, 1);
    assert_eq!(kind.source_entries.read_successful, 1);
    assert_eq!(kind.source_entries.projected, 1);
    assert_eq!(kind.source_entries.failed_selection_candidates, 2);
    assert_eq!(kind.source_entries.failed_required_reads, 0);
    let rendered = serde_json::to_value(health)?;
    let failures = rows(&rendered, "/failedEntries")?;
    assert_eq!(failures.len(), 2);
    for failure in failures {
        assert_eq!(at(failure, "/stage")?, "selection");
        assert_eq!(at(failure, "/code")?, "ambiguousIdentity");
        assert_eq!(at(failure, "/operation")?, "enumeration");
    }
    Ok(())
}

pub(super) fn dataforge() -> TestEntry {
    TestEntry::stored(
        "Data/Game2.dcb",
        DataForgeBuilder::new()
            .structure(TestStruct::new("Fixture"))
            .record(TestRecord::new(
                1,
                "Fixture",
                "Fixture.One",
                "fixture/one.xml",
            ))
            .build(),
    )
}
