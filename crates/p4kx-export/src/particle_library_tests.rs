// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{SourceInventory, SourceLocation, SourceUseRole, SourceValueState};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntry,
};
use serde_json::{Value, json};

use super::*;
use crate::catalog_read::original::with_entry_capture;
use crate::export_health_entries;
use crate::{CatalogOptions, CatalogPublicationMode, build_catalog};

#[path = "particle_library_query_tests.rs"]
mod query;
#[path = "particle_library_witness.rs"]
mod witness;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const OLD: &str = "Data/Libs/Particles/fixture-old.xml";
const NEW: &str = "Data/Libs/Particles/fixture-new.XML";
const OTHER: &str = "Data/Libs/Particles/other.xml";
const BROKEN: &str = "Data/Libs/Particles/broken.xml";
const OLD_XML: &str = "<ParticleLibrary><Particles><Params/></Particles></ParticleLibrary>";

fn synthetic_new() -> Vec<u8> {
    TestElement::new("ParticleLibrary")
        .child(
            TestElement::new("Particles")
                .child(TestElement::new("Params").attribute("RainOcclusion", "true"))
                .child(
                    TestElement::new("Childs").child(
                        TestElement::new("Particles")
                            .child(TestElement::new("Params").attribute("RainOcclusion", "0"))
                            .child(
                                TestElement::new("Params")
                                    .attribute("Texture", "fixture-texture")
                                    .attribute("RainOcclusion", "future-token"),
                            ),
                    ),
                ),
        )
        .child(
            TestElement::new("Lightning").child(
                TestElement::new("Params")
                    .attribute("StartLight", "Fixture.Start")
                    .attribute("TargetLight", "Fixture.Target")
                    .attribute("SharedLight", "Fixture.Shared"),
            ),
        )
        .child(TestElement::new("FutureElement").content("fixture body"))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

fn synthetic_archive(key_given: bool) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            DataForgeBuilder::new().version(8).build(),
        ))
        .entry(TestEntry::stored(OLD, OLD_XML))
        .entry(TestEntry::stored(NEW, synthetic_new()).encrypted())
        .entry(TestEntry::stored(OTHER, "<DifferentRoot/>"))
        .entry(TestEntry::stored(BROKEN, "<ParticleLibrary>"))
        .entry(TestEntry::stored(
            "Data/Libs/ParticlesExtra/out.xml",
            OLD_XML,
        ))
        .write_to(&path)?;
    let key = key_given.then(p4kx_testkit::test_key);
    Ok((folder, Archive::open_with_key(path, key)?))
}

#[test]
fn particle_scope_keeps_readable_siblings_and_records_failed_and_skipped_entries() -> TestResult {
    let (_folder, archive) = synthetic_archive(false)?;
    with_entry_capture(&archive, |capture| {
        let mut ledger = ExportLedger::default();
        let table = read(&archive, capture, &mut ledger, None, None)?;
        assert!(matches!(table.availability, DatasetAvailability::Available));
        assert_eq!(table.rows.len(), 1, "the readable sibling survives");
        assert_eq!(ledger.inventories, [KIND]);
        let entries = export_health_entries::compile(ledger.entries)?;
        assert_eq!(
            entries.sources.len(),
            4,
            "out-of-scope files are never opened"
        );
        assert_eq!(
            entries.failed.len(),
            2,
            "unreadable and malformed Entries are distinct"
        );
        assert_eq!(
            entries.failed[0].code,
            ExportDiagnosticCode::SourceReadFailure
        );
        assert_eq!(
            entries.failed[1].code,
            ExportDiagnosticCode::MalformedSource
        );
        assert!(
            entries
                .sources
                .iter()
                .any(|entry| { entry.disposition == SourceDisposition::SkippedAfterRead })
        );
        Ok(())
    })?;
    Ok(())
}

#[test]
fn staged_particle_rows_match_typed_bytes_and_bound_source_inventory() -> TestResult {
    let (folder, archive) = synthetic_archive(true)?;
    let scratch = tempfile::tempdir_in(folder.path())?;
    let (staged, staged_inventory) = with_entry_capture(&archive, |capture| {
        let mut ledger = ExportLedger::default();
        let table = stage(&archive, capture, &mut ledger, None, None, scratch.path())?;
        Ok((table, capture.source_uses().freeze()?))
    })?;
    let (typed, original_inventory) = with_entry_capture(&archive, |capture| {
        let mut ledger = ExportLedger::default();
        let table = read(&archive, capture, &mut ledger, None, None)?;
        Ok((table, capture.source_uses().freeze()?))
    })?;
    assert_eq!(staged.len(), typed.rows.len());
    assert_eq!(staged_inventory, original_inventory);
    let expected = serde_json::to_vec(&typed)?;
    assert_eq!(serde_json::to_vec(&staged)?, expected, "counting replay");
    assert_eq!(serde_json::to_vec(&staged)?, expected, "writing replay");
    drop(staged);
    assert_eq!(scratch.path().read_dir()?.count(), 0);
    Ok(())
}

#[test]
fn particle_wire_preserves_order_tokens_and_per_datum_physical_provenance() -> TestResult {
    let (_folder, archive) = synthetic_archive(true)?;
    let (table, inventory) = with_entry_capture(&archive, |capture| {
        let mut ledger = ExportLedger::default();
        let table = read(&archive, capture, &mut ledger, None, None)?;
        Ok((table, capture.source_uses().freeze()?))
    })?;
    assert_eq!(
        table.rows.len(),
        2,
        "one row per physical source in directory order"
    );
    let old = table.rows.first().ok_or("plaintext particle library")?;
    let new = table.rows.get(1).ok_or("encrypted particle library")?;
    let old_rain = rain_slots(old).next().ok_or("old parameter")?.1;
    assert_eq!(old_rain.state(), SourceValueState::Absent);
    assert_eq!(old_rain.origin().contains_encrypted_source(), Some(false));
    let new_names: Vec<_> = new
        .nodes
        .iter()
        .filter_map(|node| node.name.value().map(String::as_str))
        .collect();
    assert_eq!(
        new_names,
        [
            "ParticleLibrary",
            "Particles",
            "Params",
            "Childs",
            "Particles",
            "Params",
            "Params",
            "Lightning",
            "Params",
            "FutureElement"
        ]
    );
    assert_eq!(
        new.nodes
            .get(5)
            .and_then(|node| node.parent_node_index.value()),
        Some(&4)
    );

    assert_rain_slots(new, &inventory)?;
    assert_original_payloads(new)
}

fn assert_rain_slots(new: &ParticleLibrary, inventory: &SourceInventory) -> TestResult {
    let new_rain: Vec<_> = rain_slots(new).collect();
    assert_eq!(
        new_rain
            .iter()
            .map(|(_, rain)| (rain.state(), rain.value()))
            .collect::<Vec<_>>(),
        [
            (SourceValueState::Present, Some(&true)),
            (SourceValueState::Present, Some(&false)),
            (SourceValueState::Invalid, None),
        ]
    );
    for ((node, rain), ordinal) in new_rain.into_iter().zip([0, 0, 1]) {
        let use_id = rain
            .origin()
            .source_uses_refs()
            .first()
            .ok_or("typed attribute use")?;
        let used = inventory
            .uses()
            .iter()
            .find(|used| used.id() == use_id)
            .ok_or("resolved use")?;
        assert_eq!(rain.origin().contains_encrypted_source(), Some(true));
        assert!(rain.origin().source_set_complete());
        assert_eq!(used.role(), SourceUseRole::Conversion);
        assert_eq!(
            used.location(),
            &SourceLocation::Subresource {
                namespace: "xml".to_owned(),
                indices: vec![
                    u64::from(*node.node_index.value().ok_or("node index")?),
                    ordinal
                ],
                field: "@RainOcclusion".to_owned(),
            }
        );
    }
    Ok(())
}

fn assert_original_payloads(new: &ParticleLibrary) -> TestResult {
    let raw = |index: usize| {
        new.nodes
            .get(index)
            .into_iter()
            .flat_map(|node| &node.attributes)
            .filter_map(|attribute| {
                Some((
                    attribute.name.value()?.as_str(),
                    attribute.value.value()?.as_str(),
                ))
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        raw(6),
        [
            ("Texture", "fixture-texture"),
            ("RainOcclusion", "future-token")
        ]
    );
    assert_eq!(
        raw(8),
        [
            ("StartLight", "Fixture.Start"),
            ("TargetLight", "Fixture.Target"),
            ("SharedLight", "Fixture.Shared")
        ]
    );
    let future = new.nodes.last().ok_or("unknown source element")?;
    assert_eq!(
        future
            .text
            .first()
            .and_then(|value| value.value())
            .map(String::as_str),
        Some("fixture body")
    );
    assert_eq!(future.name.origin().contains_encrypted_source(), Some(true));
    Ok(())
}

#[test]
fn nested_particle_payload_keeps_outer_encryption_and_inner_physical_identity() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let container = ArchiveBuilder::new()
        .entry(TestEntry::stored(OLD, OLD_XML))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("fixture-container.zip", container).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let host = archive.entries().first().ok_or("encrypted parent Entry")?;
    let container = archive.open_container(host)?;
    let entry = container.entries().first().ok_or("plain child Entry")?;
    let (row, inventory) = with_entry_capture(&archive, |capture| {
        let source = EntrySource::inner(capture, &archive, host, &container, entry)?;
        let Observation::Projected(row) = observe(&container, entry, &source, capture, None)?
        else {
            return Err(crate::ExportAccountingError::ConflictingSourceDatum.into());
        };
        Ok((row, capture.source_uses().freeze()?))
    })?;
    let rain = rain_slots(&row).next().ok_or("observed parameter")?.1;
    assert_eq!(rain.origin().contains_encrypted_source(), Some(true));
    let id = rain
        .origin()
        .source_uses_refs()
        .first()
        .ok_or("captured selector")?;
    let usage = inventory
        .uses()
        .iter()
        .find(|usage| usage.id() == id)
        .ok_or("resolved selector")?;
    assert_eq!(
        usage.source(),
        &row.identity.input_ref,
        "the datum addresses the plain child"
    );
    let source = inventory
        .sources()
        .iter()
        .find(|source| source.id() == usage.source())
        .ok_or("physical child descriptor")?;
    let source = serde_json::to_value(source)?;
    assert_eq!(source["ancestors"].as_array().map(Vec::len), Some(1));
    assert_eq!(source["storedEncryption"], serde_json::json!("unencrypted"));
    assert_eq!(
        usage.effective_encryption(),
        p4kx_extraction::SourceEncryptionState::Encrypted
    );
    Ok(())
}

#[test]
fn common_catalog_build_serializes_particle_health_and_diagnostic_siblings() -> TestResult {
    let (_folder, archive) = synthetic_archive(true)?;
    let normal = CatalogOptions::new(UNIX_EPOCH);
    assert!(matches!(
        build_catalog(&archive, &normal),
        Err(CatalogError::IncompleteExport(_))
    ));
    let mut diagnostic = normal;
    diagnostic.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &diagnostic)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let rows = catalog
        .pointer("/definitions/particleLibraries/rows")
        .and_then(Value::as_array)
        .ok_or("serialized part of schema v3")?;
    assert_eq!(rows.len(), 2);
    let kinds = catalog
        .pointer("/exportHealth/kinds")
        .and_then(Value::as_array)
        .ok_or("catalog read/projection health")?;
    let health = kinds
        .iter()
        .find(|kind| kind.get("kind") == Some(&json!("particleLibraries")))
        .ok_or("health for particle library Entries")?;
    assert_eq!(health.get("sourceMode"), Some(&json!("archiveEntries")));
    assert_eq!(health.get("writtenRows"), Some(&json!(2)));
    assert_eq!(
        health.pointer("/sourceEntries/failedProjections"),
        Some(&json!(1))
    );
    assert_eq!(
        catalog.pointer("/completeness/particleLibraries/status"),
        Some(&json!("complete"))
    );
    let filter = Filter::new("fixture-old.xml");
    diagnostic.filter = Some(&filter);
    let filtered = build_catalog(&archive, &diagnostic)?;
    let filtered: Value = serde_json::from_slice(filtered.json())?;
    assert_eq!(
        filtered
            .pointer("/definitions/particleLibraries/rows")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        filtered.pointer("/completeness/particleLibraries/status"),
        Some(&json!("partial"))
    );
    Ok(())
}

fn rain_slots(
    library: &ParticleLibrary,
) -> impl Iterator<
    Item = (
        &ParticleLibraryElement,
        &p4kx_extraction::SourcedValue<bool>,
    ),
> {
    library
        .nodes
        .iter()
        .filter_map(|node| Some((node, node.rain_occlusion.as_ref()?)))
}
