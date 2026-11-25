// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_export::catalog_queries::loot::{
    self, LootAncestorSelection, LootContainerQuery, LootContainerStatus,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};
use p4kx_export::{
    CatalogError, CatalogPublicationMode, Compression, ExportHealth, ExportHealthStatus,
    ExportStageStatus, Kind, build_catalog_from, write_catalog_from,
};
use p4kx_extraction::{ProvenanceMode, SourceUseRole};
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use crate::health_origin_assertions as proof;
use crate::health_origin_entry_source as entries;
use crate::health_origin_fixture::{self as fixture, Fixture, TestResult};

const SYSTEM: &str = "alpha";

#[test]
fn unreadable_selected_root_preserves_upstream_failure_and_unavailable_loot_document_scope()
-> TestResult {
    let complete = source(false)?;
    let previous = build_catalog_from(
        &complete.archive,
        &complete.entry,
        fixture::options().with_full_provenance(),
    )?;
    assert!(
        previous.summary().export_health().is_complete(),
        "readable counterpart is complete"
    );
    let previous_catalog: Value = serde_json::from_slice(previous.json())?;
    verify_scope(&previous_catalog, "absent", 1)?;
    let source = source(true)?;
    let root = entries::original(&source.archive, &entries::outer(SYSTEM))?;
    assert!(
        root.is_encrypted(),
        "the actual selected root payload is encrypted"
    );
    assert!(
        source.archive.open_container(root).is_err(),
        "the intended selected physical root really cannot be opened"
    );
    let (partial, catalog) = source.diagnostic()?;
    verify_health(partial.summary().export_health())?;
    verify_scope(&catalog, "unavailable", 0)?;
    verify_failures(&catalog, root.index())?;
    proof::closure(&catalog)?;
    assert!(
        matches!(
            build_catalog_from(
                &source.archive,
                &source.entry,
                fixture::options().with_full_provenance()
            ),
            Err(CatalogError::IncompleteExport(_))
        ),
        "unreadable original refuses Normal publication"
    );
    verify_refusal(&source);
    Ok(())
}

#[test]
fn loot_query_keeps_encrypted_root_failure_without_marker_proof_refs() -> TestResult {
    let fixture = source(true)?;
    let root = entries::original(&fixture.archive, &entries::outer(SYSTEM))?;
    let query = LootContainerQuery::new(
        u32::try_from(root.index())?,
        fixture::guid(0x5712).parse()?,
        "Data/ObjectContainers/PU/synthetic/loottarget.socpak",
        LootAncestorSelection::WithoutResolvedObject,
    );
    let markers = loot::read_loot_container(&fixture.archive, query, None)?;
    let full = CatalogQuerySession::from_entry(
        &fixture.archive,
        &fixture.entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?
    .with_provenance_mode(ProvenanceMode::Full)
    .read_loot_container(query, None)?;

    assert_eq!(markers.status, LootContainerStatus::Unavailable);
    assert_eq!(markers.status, full.status);
    assert_eq!(markers.records, full.records);
    let marker_survey = markers.placement_survey.ok_or("marker placement survey")?;
    let full_survey = full.placement_survey.ok_or("full placement survey")?;
    assert!(!marker_survey.source_set_complete());
    assert_eq!(
        marker_survey.source_set_complete(),
        full_survey.source_set_complete()
    );
    assert_eq!(
        marker_survey.contains_encrypted_source(),
        full_survey.contains_encrypted_source()
    );
    assert!(
        markers.failed_source_uses.is_empty(),
        "marker result omits physical attempts"
    );
    assert_eq!(markers.source_inventory.iter_uses().count(), 0);
    assert!(
        !full.failed_source_uses.is_empty(),
        "fixture must report a real failed attempt"
    );
    for attempt in &full.failed_source_uses {
        assert!(
            full.source_inventory
                .iter_uses()
                .any(|used| { used.id() == attempt && used.role() == SourceUseRole::Attempted }),
            "the full proof contains each original attempt"
        );
    }
    Ok(())
}

fn source(unreadable: bool) -> TestResult<Fixture> {
    let root = if unreadable {
        TestEntry::stored(entries::outer(SYSTEM), b"not a nested Archive").encrypted()
    } else {
        entries::container(SYSTEM, b"<Entity/>", true)
    };
    entries::with_starmap(vec![
        root,
        TestEntry::stored(fixture::REGISTRY, b"<LayerBackups/>"),
    ])
}

fn verify_scope(catalog: &Value, availability: &str, root_instances: usize) -> TestResult {
    assert_eq!(
        proof::rows(catalog, "/definitions/starmapObjects/rows")?.len(),
        1,
        "one actual selected original Starmap Record"
    );
    assert_eq!(
        proof::rows(catalog, "/placements/containerInstances/rows")?.len(),
        root_instances,
        "a failed root does not fabricate a resolved placement"
    );
    assert_eq!(
        proof::at(
            catalog,
            "/declarations/lootEntityContainerDocuments/availability"
        )?,
        &json!(availability),
        "failed source inspection cannot certify document absence"
    );
    assert_eq!(
        proof::rows(catalog, "/declarations/lootEntityContainerDocuments/rows")?,
        &[] as &[Value],
        "no original document is invented from unreadable bytes"
    );
    assert_eq!(
        proof::rows(catalog, "/lootContainers")?,
        &[] as &[Value],
        "no Loot placement is invented"
    );
    Ok(())
}

fn verify_health(health: &ExportHealth) -> TestResult {
    assert_eq!(
        health.publication_mode,
        CatalogPublicationMode::DiagnosticPartial,
        "only DiagnosticPartial publishes the incomplete result"
    );
    assert_eq!(
        health.status,
        ExportHealthStatus::Incomplete,
        "overall source failure"
    );
    assert_eq!(
        health.read,
        ExportStageStatus::Incomplete,
        "original read failure"
    );
    for kind in [Kind::StarmapObjects, Kind::LootContainers, Kind::Shops] {
        verify_failed_kind(health, kind)?;
    }
    for report in health.kinds().iter().filter(|row| {
        !matches!(
            row.kind,
            Kind::StarmapObjects | Kind::LootContainers | Kind::Shops
        )
    }) {
        assert_eq!(
            report.read,
            ExportStageStatus::Complete,
            "{:?}",
            report.kind
        );
        assert_eq!(
            report.projection,
            ExportStageStatus::Complete,
            "{:?}",
            report.kind
        );
    }
    Ok(())
}

fn verify_failed_kind(health: &ExportHealth, kind: Kind) -> TestResult {
    let report = health
        .kinds()
        .iter()
        .find(|row| row.kind == kind)
        .ok_or("affected kind")?;
    assert_eq!(
        report.read,
        ExportStageStatus::Incomplete,
        "{kind:?}: original read fails"
    );
    assert_eq!(
        report.source_entries.failed_dependencies, 1,
        "{kind:?}: one failed root dependency"
    );
    let selected_failures = usize::from(kind == Kind::LootContainers);
    assert_eq!(
        report.failed_entries().len(),
        1 + selected_failures,
        "{kind:?}: exact failure count"
    );
    if kind == Kind::Shops {
        assert!(
            !report.source_inventory_complete,
            "unreadable root prevents the Shop census"
        );
    }
    assert_eq!(
        report.source_entries.failed_required_reads, selected_failures,
        "only Loot additionally selects this original root"
    );
    Ok(())
}

fn verify_failures(catalog: &Value, index: usize) -> TestResult {
    for (kind, expected) in [("starmapObjects", 1), ("lootContainers", 2), ("shops", 1)] {
        let failures = entries::failure(catalog, kind, "containerDirectory")?;
        assert_eq!(
            failures.len(),
            expected,
            "selected and dependency failures stay distinct"
        );
        for row in failures {
            proof::present(row, "entryIndex", &json!(index), Some(false))?;
            let path = proof::present_text(
                catalog,
                row,
                "path",
                &json!(entries::outer(SYSTEM)),
                Some(false),
            )?;
            assert_eq!(
                entries::metadata_paths(catalog, path)?,
                [entries::outer(SYSTEM)],
                "failure identity cites exactly the original root"
            );
            assert_eq!(
                row.get("stage"),
                Some(&json!("sourceRead")),
                "original read stage"
            );
            proof::independent(proof::cell(row, "containerEntryIndex", "absent")?)?;
        }
    }
    assert!(
        proof::rows(catalog, "/provenance/uses")?
            .iter()
            .any(|used| {
                used.get("scope") == Some(&json!("payload"))
                    && used.get("role") == Some(&json!("attempted"))
                    && proof::source(catalog, used).is_ok_and(|source| {
                        source.pointer("/path/value") == Some(&json!(entries::outer(SYSTEM)))
                            && source.get("storedEncryption") == Some(&json!("encrypted"))
                    })
            }),
        "the actual failed encrypted payload remains an attempt, separate from its plain identity"
    );
    Ok(())
}

fn verify_refusal(source: &Fixture) {
    let called = Cell::new(false);
    let result = write_catalog_from(
        &source.archive,
        &source.entry,
        fixture::options().with_full_provenance(),
        Compression::None,
        || {
            called.set(true);
            Ok(Vec::<u8>::new())
        },
    );
    assert!(
        matches!(result, Err(CatalogError::IncompleteExport(_))),
        "Normal output is refused"
    );
    assert!(
        !called.get(),
        "failed source Health refuses before creating any output"
    );
}
