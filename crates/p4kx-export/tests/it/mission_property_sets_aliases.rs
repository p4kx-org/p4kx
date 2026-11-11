// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Conflicting physical Generator aliases cannot publish an ambiguous canonical join.

use std::collections::BTreeSet;
use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{
    BuiltCatalog, CatalogError, CatalogOptions, CatalogPublicationMode, ExportDiagnosticCode, Kind,
    build_catalog,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestValue};
use serde_json::Value;

use crate::mission_property_sets_fixture::{generator, parameters, rows};
use crate::scoped_fixture::{self, TestResult, property};

fn source(value: bool) -> [TestValue; 3] {
    [
        parameters(vec![property(
            "Flag",
            TestInstance::new("MissionPropertyValue_Boolean")
                .value("value", TestValue::Bool(value)),
        )]),
        parameters(vec![]),
        parameters(vec![]),
    ]
}

fn build(values: [bool; 2], mode: CatalogPublicationMode) -> Result<BuiltCatalog, Box<dyn Error>> {
    let mut builder = scoped_fixture::schema(DataForgeBuilder::new().version(8));
    for value in values {
        builder = builder.record(generator(source(value)));
    }
    let root = tempfile::tempdir()?;
    let path = root.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", builder.build()))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let options = CatalogOptions {
        publication_mode: mode,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    Ok(build_catalog(&archive, &options)?)
}
#[test]
fn conflicting_physical_generators_refuse_normal_publication() -> TestResult {
    let error = build([false, true], CatalogPublicationMode::Normal)
        .err()
        .ok_or("normal publication accepted ambiguous source identity")?;
    let error = *error.downcast::<CatalogError>()?;
    let CatalogError::IncompleteExport(refusal) = error else {
        return Err(error.into());
    };
    let health = refusal.health();
    for kind in [Kind::Contracts, Kind::MissionPropertySets] {
        let indices: BTreeSet<_> = health
            .failed_records()
            .iter()
            .filter(|source| {
                source.kind == Some(kind) && source.code == ExportDiagnosticCode::AmbiguousIdentity
            })
            .map(|source| {
                source
                    .record_index()
                    .value()
                    .copied()
                    .ok_or("failed physical Record position")
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        assert_eq!(
            indices,
            BTreeSet::from([0, 1]),
            "both physical alias owners are retained for {kind}"
        );
    }
    Ok(())
}

fn verify_quarantined_health(health: &p4kx_export::ExportHealth) -> TestResult {
    let kind = health
        .kinds()
        .iter()
        .find(|kind| kind.kind == Kind::MissionPropertySets)
        .ok_or("set health")?;
    assert_eq!(
        kind.source_records.projected, 2,
        "both canonical owner reads and typed source projections succeeded"
    );
    assert_eq!(
        kind.source_records.omitted_after_projection_failure, 0,
        "partial retained owners are not whole omitted sources"
    );
    assert_eq!(
        kind.failed_derived_projections, 2,
        "each original physical owner has one failed merge operation"
    );
    assert!(
        health
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.warning_code.as_deref()
                == Some("ambiguous_mission_property_set")),
        "a stable diagnostic identifies the canonical merge failure"
    );
    Ok(())
}

#[test]
fn diagnostic_quarantines_only_conflicting_sets_and_all_their_joins() -> TestResult {
    let baseline: Value =
        serde_json::from_slice(build([false, false], CatalogPublicationMode::Normal)?.json())?;
    let baseline_sets = rows(&baseline)?;
    let handler = baseline_sets
        .iter()
        .find(|set| set["level"] == "handler")
        .ok_or("handler")?;
    let built = build([false, true], CatalogPublicationMode::DiagnosticPartial)?;
    let document: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        rows(&document)?.len(),
        3,
        "the three compatible authored arrays remain"
    );
    for set in rows(&document)? {
        assert_ne!(
            set["guid"], handler["guid"],
            "the incompatible canonical identity is quarantined"
        );
    }
    for contract in crate::catalog_contracts::v3::rows(&document)? {
        for guid in contract["propertySetGuids"]
            .as_array()
            .ok_or("propertySetGuids")?
        {
            assert_ne!(
                guid, &handler["guid"],
                "no surviving contract resolves the ambiguous identity"
            );
            assert!(
                rows(&document)?.iter().any(|set| &set["guid"] == guid),
                "every retained join resolves uniquely"
            );
        }
    }
    verify_quarantined_health(built.summary().export_health())?;
    Ok(())
}

#[test]
fn identical_alias_payloads_keep_one_canonical_set_per_authored_location() -> TestResult {
    let built = build([true, true], CatalogPublicationMode::Normal)?;
    let document: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        rows(&document)?.len(),
        4,
        "compatible physical aliases are idempotent"
    );
    assert!(
        !built
            .summary()
            .export_health()
            .failed_records()
            .iter()
            .any(|source| source.code == ExportDiagnosticCode::AmbiguousIdentity),
        "compatible payloads do not invent an identity failure"
    );
    Ok(())
}
