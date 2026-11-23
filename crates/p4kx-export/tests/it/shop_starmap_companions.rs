// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::shops::read_shop;
use p4kx_export::{
    CatalogError, CatalogOptions, CatalogPublicationMode, EntryOperation, ExportDiagnosticStage,
    ExportHealth, ExportSourceRole, Kind, StarmapTraversalBudget, build_catalog,
};
use p4kx_extraction::{Archive, PhysicalSourceKind};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestElement, TestEntry};
use serde_json::{Value, json};

use super::batch::physical_registry_nodes;
use super::{LOCATIONS, PLANET, REGISTRY, ROOT, Route, TestResult, archive, shop};
use crate::health_origin_assertions as health;
use crate::shops_fixture;
use crate::starmap_fixture;

const LOCATION_GUID: &str = "10000000-0000-4000-8000-000000000018";
const HCD: &str = "hcd/10000000-0000-4000-8000-000000000018.xml";
const ENTITY: &str = "entdata/location.entxml";

#[derive(Clone, Copy)]
enum Fault {
    Malformed,
    Unreadable,
}

#[test]
fn independent_companion_failures_stay_in_export_health_and_leave_shop_evidence_complete()
-> TestResult {
    assert_companion_scope(None, None)?;
    for fault in [Fault::Malformed, Fault::Unreadable] {
        assert_companion_scope(Some(fault), None)?;
        assert_companion_scope(None, Some(fault))?;
    }
    Ok(())
}

fn assert_companion_scope(hcd_fault: Option<Fault>, entity_fault: Option<Fault>) -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = with_companions(directory.path(), hcd_fault, entity_fault)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let output = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(output.json())?;
    let [location] = health::rows(&catalog, "/placements/locationInstances/rows")? else {
        return Err("one authored location occurrence with both companion lookups".into());
    };
    assert_companion_fields(location, hcd_fault, entity_fault)?;
    assert_failure(output.summary().export_health(), HCD, hcd_fault);
    assert_failure(output.summary().export_health(), ENTITY, entity_fault);
    assert_shop_evidence(&archive, &catalog, true)?;
    if hcd_fault.is_some() || entity_fault.is_some() {
        let Err(CatalogError::IncompleteExport(refusal)) =
            build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH))
        else {
            return Err("strict publication must retain the real companion failure".into());
        };
        assert_failure(refusal.health(), HCD, hcd_fault);
        assert_failure(refusal.health(), ENTITY, entity_fault);
    }
    Ok(())
}

#[test]
fn unreadable_main_descriptor_still_prevents_a_complete_shop_name_survey() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = archive(directory.path(), None, false, Route::UnreadableChild)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let output = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(output.json())?;
    assert_shop_evidence(&archive, &catalog, false)
}

fn assert_shop_evidence(archive: &Archive, catalog: &Value, complete: bool) -> TestResult {
    let [_, selected, _] = physical_registry_nodes(archive)?;
    let sdk = read_shop(archive, selected, StarmapTraversalBudget::new(), None)?;
    let coverage = sdk
        .coverage
        .names
        .as_ref()
        .ok_or("the place requires a name survey")?;
    assert_eq!(
        coverage.routes_complete, complete,
        "the real placement route controls the name proof"
    );
    let row = sdk.row.as_ref().ok_or("the admitted original Shop")?;
    let cell = health::cell(shop(catalog, LOCATIONS[1])?, "starmapObjectGuid", "present")?;
    assert_eq!(
        cell["value"],
        json!(PLANET),
        "the observed match is retained"
    );
    assert_eq!(
        cell["containsEncryptedSource"], true,
        "the observed Child XML remains a protected contributor"
    );
    assert_eq!(
        cell["sourceSetComplete"], complete,
        "the original name evidence is complete exactly when its own source paths are complete"
    );
    let sdk_inventory = json!({"provenance": sdk.source_inventory});
    assert_eq!(
        health::with_origins(
            serde_json::to_value(&row.starmap_object_guid)?,
            &sdk_inventory
        )?,
        health::with_origins(cell.clone(), catalog)?,
        "the SDK and Normal's Shop projection use the same exact original name evidence",
    );
    for source in sdk.source_inventory.iter_sources() {
        if let PhysicalSourceKind::ArchiveEntry(entry) = source.kind() {
            assert!(
                entry.path().value().is_none_or(
                    |path| !matches_companion(path, HCD) && !matches_companion(path, ENTITY)
                ),
                "the complete lookup opens no supplemental location member",
            );
        }
    }
    health::closure(catalog)
}

fn assert_companion_fields(
    location: &Value,
    hcd: Option<Fault>,
    entity: Option<Fault>,
) -> TestResult {
    let hcd_state = match hcd {
        None => "present",
        Some(Fault::Malformed) => "invalid",
        Some(Fault::Unreadable) => "unreadable",
    };
    assert_eq!(
        health::at(location, "/declaration/hierarchicalContext/document/state")?,
        &json!(hcd_state),
        "the full projection retains the actual original HCD field state",
    );
    let entity_state = if entity.is_none() {
        "present"
    } else {
        "unreadable"
    };
    assert_eq!(
        health::at(location, "/declaration/entXml/selected/state")?,
        &json!(entity_state),
        "the full projection retains the actual ENTXML admission state",
    );
    Ok(())
}

fn assert_failure(health: &ExportHealth, path: &str, fault: Option<Fault>) {
    let expected = fault.map(|fault| match fault {
        Fault::Malformed => ExportDiagnosticStage::Projection,
        Fault::Unreadable => ExportDiagnosticStage::SourceRead,
    });
    let actual: Vec<_> = health
        .failed_entries()
        .iter()
        .filter(|failed| {
            failed.kind() == Kind::StarmapObjects
                && failed
                    .path_text()
                    .value()
                    .is_some_and(|value| matches_companion(value, path))
        })
        .map(|failed| {
            (
                failed.kind(),
                failed.operation(),
                failed.role(),
                failed.stage(),
            )
        })
        .collect();
    let expected: Vec<_> = expected
        .into_iter()
        .map(|stage| {
            (
                Kind::StarmapObjects,
                EntryOperation::SourceXml,
                ExportSourceRole::Dependency,
                stage,
            )
        })
        .collect();
    assert_eq!(
        actual, expected,
        "the original companion error retains its own required operation"
    );
}

fn matches_companion(value: &str, path: &str) -> bool {
    value
        .replace('\\', "/")
        .to_ascii_lowercase()
        .ends_with(path)
}

fn with_companions(
    dir: &Path,
    hcd_fault: Option<Fault>,
    entity_fault: Option<Fault>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let child = starmap_fixture::child("100,0,0", Some(starmap_fixture::PLANET), "planet_hurston")
        .attribute("locationType", "3")
        .attribute("guid", LOCATION_GUID)
        .attribute("hasHierarchicalContextData", "true");
    let root = TestElement::new("ObjectContainer").child(starmap_fixture::children(vec![child]));
    let context = TestElement::new("HierarchicalContextData")
        .attribute("__version", "2")
        .attribute("preset", PLANET);
    let entity = TestElement::new("Entity")
        .attribute("EntityCryGUID", LOCATION_GUID)
        .attribute("EntityClassGUID", PLANET);
    let system = TestContainer::new()
        .entry(TestEntry::stored("stantonsystem.xml", root.to_xml()))
        .entry(companion(HCD, &context, hcd_fault))
        .entry(companion(ENTITY, &entity, entity_fault))
        .build();
    let path = dir.join("Data.p4k");
    ArchiveBuilder::v2()
        .entry(
            TestEntry::stored("Data\\Game2.dcb", starmap_fixture::database().build())
                .with_encryption_field(0),
        )
        .entry(TestEntry::stored(REGISTRY, shops_fixture::registry()).with_encryption_field(0))
        .entry(TestEntry::stored(ROOT, system).encrypted())
        .write_to(&path)?;
    Ok(path)
}

fn companion(path: &str, healthy: &TestElement, fault: Option<Fault>) -> TestEntry {
    match fault {
        None => TestEntry::stored(path, healthy.to_xml()),
        Some(Fault::Malformed) => TestEntry::stored(path, b"<unclosed".as_slice()),
        Some(Fault::Unreadable) => TestEntry::stored(path, healthy.to_xml()).with_crc32(0),
    }
}
