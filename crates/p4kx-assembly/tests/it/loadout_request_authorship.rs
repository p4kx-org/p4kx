// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::Ordering;

use p4kx_archive::Archive;
use p4kx_assembly::{ArchiveFiles, Entities, LoadoutFiles, LoadoutResolver, LoadoutXmlDecoding};
use p4kx_dataforge::DataForge;
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntity,
    TestEntry,
};

use crate::loadout_original_controls_support as original;
use crate::loadout_request_authorship_support as request_source;
use crate::loadout_request_authorship_support::{EventKind, Probe, State, TestResult};

#[path = "loadout_request_vehicle_assertions.rs"]
mod vehicle_assertions;

use self::vehicle_assertions::verify_vehicle_receipts;

#[test]
fn equal_xml_paths_keep_first_document_receipt_and_distinct_current_callers() -> TestResult {
    let bytes = original::database();
    let forge = DataForge::open(&bytes)?;
    let files = original::GenericFiles::default();
    let probe = Arc::new(Probe::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), probe.clone())
            .with_files(&files);
    for record in forge.records() {
        resolver.resolve(&record)?;
    }
    let state = probe
        .0
        .lock()
        .map_err(|_| "request evidence mutex is poisoned")?;
    let reads: Vec<_> = state
        .events
        .iter()
        .filter(|event| event.kind == EventKind::Read)
        .collect();
    let reused: Vec<_> = state
        .events
        .iter()
        .filter(|event| event.kind == EventKind::Reused)
        .collect();
    assert_eq!(
        reads.len(),
        1,
        "equal text still performs one physical file read"
    );
    assert_eq!(
        reused.len(),
        1,
        "the second caller receives the original document result"
    );
    let first = reads.first().ok_or("the actual read is absent")?;
    let second = reused.first().ok_or("the actual reuse is absent")?;
    assert!(
        first.observation.is_same_operation(&second.observation),
        "reuse preserves the unchanged intrinsic document operation"
    );
    assert_eq!(
        first.request.selection(),
        second.request.selection(),
        "the cached failure path retains the actual first-population candidate"
    );
    request_source::assert_current_caller(&state, first, 0)?;
    request_source::assert_current_caller(&state, second, 1)?;
    assert!(
        !first.observation.source().is_missing_archive(),
        "a generic provider remains distinct from a missing Archive Entry"
    );
    drop(state);
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        1,
        "no second provider read is introduced"
    );
    Ok(())
}

fn include_bytes(binary: bool) -> Vec<u8> {
    if binary {
        TestElement::new("Loadout")
            .child(
                TestElement::new("Loadouts").child(
                    TestElement::new("Loadout").attribute("loadout", "synthetic/&missing.xml"),
                ),
            )
            .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
    } else {
        b"\xef\xbb\xbf<Loadout><Loadouts><Loadout loadout=\"synthetic/&amp;missing.xml\"/></Loadouts></Loadout>".to_vec()
    }
}

fn verify_include(binary: bool) -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive_path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            format!("Data/{}", original::CONFIGURED),
            include_bytes(binary),
        ))
        .write_to(&archive_path)?;
    let archive = Archive::open(archive_path)?;
    let database = original::database();
    let forge = DataForge::open(&database)?;
    let files = ArchiveFiles::new(&archive);
    let probe = Arc::new(Probe::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), probe.clone())
            .with_files(&files);
    resolver.resolve(&forge.record(0).ok_or("the root Record is absent")?)?;
    let state = probe
        .0
        .lock()
        .map_err(|_| "XML evidence mutex is poisoned")?;
    verify_include_receipts(&archive, &state, binary)
}

fn verify_include_receipts(archive: &Archive, state: &State, binary: bool) -> TestResult {
    assert_eq!(
        state.xml.len(),
        1,
        "the real include attribute is observed once"
    );
    let site = state.xml.first().ok_or("the include site is absent")?;
    let root_read = state
        .events
        .iter()
        .find(|event| {
            event.kind == EventKind::Read && event.observation.source().entry_origin().is_some()
        })
        .ok_or("the root read is absent")?;
    let missing = state
        .events
        .iter()
        .find(|event| {
            event.kind == EventKind::Read && event.observation.source().is_missing_archive()
        })
        .ok_or("the missing include read is absent")?;
    assert!(
        site.observation.is_same_operation(&root_read.observation),
        "the include borrows the genuine original read used by that same parse"
    );
    assert_eq!(
        site.node,
        if binary { 2 } else { 3 },
        "the original decoder's actual node index is retained"
    );
    assert_eq!(
        site.name, "loadout",
        "the actual winning attribute name is retained"
    );
    assert_eq!(
        site.value, "synthetic/&missing.xml",
        "the actual attribute decode supplies the path"
    );
    assert_eq!(
        missing.request.selection(),
        Some(&site.selection),
        "the include's whole private pair reaches its real read"
    );
    assert!(
        missing.observation.source().entry_origin().is_none(),
        "a missing Archive target never invents an EntryOrigin"
    );
    assert!(
        archive.owns_origin(
            site.observation
                .source()
                .entry_origin()
                .ok_or("the parsed source EntryOrigin is absent")?
        ),
        "the attribute source belongs to the actual source Archive"
    );
    verify_xml_decoder(site, binary);
    Ok(())
}

fn verify_xml_decoder(site: &request_source::XmlSite, binary: bool) {
    if binary {
        assert!(
            matches!(
                site.decoding,
                LoadoutXmlDecoding::CryXml {
                    repaired_utf8: false,
                    ..
                }
            ),
            "the binary decoder retains its actual repair outcome"
        );
    } else {
        assert!(
            matches!(site.decoding, LoadoutXmlDecoding::Text { utf8_bom: true }),
            "the actual leading UTF-8 BOM removal is retained"
        );
    }
}

#[test]
fn text_include_receipts_borrow_the_actual_parse_and_read() -> TestResult {
    verify_include(false)
}

#[test]
fn cryxml_include_receipts_borrow_the_actual_parse_and_read() -> TestResult {
    verify_include(true)
}

#[test]
fn vehicle_patch_cache_keeps_actual_candidates_and_current_record_getters() -> TestResult {
    let fixture = vehicle_fixture()?;
    let forge = DataForge::open(&fixture.database)?;
    let files = ArchiveFiles::new(&fixture.archive);
    let probe = Arc::new(Probe::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), probe.clone())
            .with_files(&files);
    for record in forge.records() {
        resolver.resolve(&record)?;
    }
    let state = probe
        .0
        .lock()
        .map_err(|_| "vehicle evidence mutex is poisoned")?;
    verify_vehicle_receipts(&state, fixture.patch)
}

struct VehicleFixture {
    archive: Archive,
    database: Vec<u8>,
    patch: &'static str,
    _directory: tempfile::TempDir,
}

fn vehicle_fixture() -> Result<VehicleFixture, Box<dyn std::error::Error>> {
    let definition = "Scripts/Entities/Vehicles/Implementations/Xml/shared.xml";
    let patch = "Scripts/Entities/Vehicles/Implementations/Xml/Modifications/shared_patch.xml";
    let directory = tempfile::tempdir()?;
    let archive_path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(format!("Data/{definition}"),
            b"<Vehicle><Modifications><Modification name=\"test\" patchFile=\"Modifications/shared_patch\"/></Modifications></Vehicle>".to_vec()))
        .entry(TestEntry::stored(format!("Data/{patch}"), b"<broken>".to_vec()))
        .write_to(&archive_path)?;
    let archive = Archive::open(archive_path)?;
    let mut builder = p4kx_testkit::entity_schema(DataForgeBuilder::new());
    for index in 0..2_u128 {
        builder = builder.record(
            TestEntity::new(index + 1, "SharedVehicle", format!("synthetic/{index}.xml"))
                .vehicle(format!("  {definition}  "), " test ")
                .record(),
        );
    }
    let database = builder.build();
    Ok(VehicleFixture {
        archive,
        database,
        patch,
        _directory: directory,
    })
}

#[test]
fn missing_archive_lookup_retains_its_actual_input_without_an_entry() -> TestResult {
    let directory = tempfile::tempdir()?;
    let left_path = directory.path().join("left.p4k");
    let right_path = directory.path().join("right.p4k");
    for path in [&left_path, &right_path] {
        ArchiveBuilder::new()
            .entry(TestEntry::stored("Data/marker.xml", b"<Loadout/>".to_vec()))
            .write_to(path)?;
    }
    let left = Archive::open(left_path)?;
    let right = Archive::open(right_path)?;
    let missing = ArchiveFiles::new(&left).read_observed("synthetic/missing.xml");
    let input = missing
        .source()
        .archive_input_origin()
        .ok_or("the actual missing lookup's Archive input is absent")?;
    assert!(
        input.is_same_input(left.input_origin()),
        "an absent Entry retains the Archive that performed its real lookup"
    );
    assert!(
        !input.is_same_input(right.input_origin()),
        "an equal configured path cannot certify absence in another Archive input"
    );
    assert!(
        missing.source().entry_origin().is_none(),
        "an absent Entry must not invent an EntryOrigin"
    );
    assert!(
        missing.source().is_missing_archive(),
        "the real absent Archive lookup is explicit"
    );
    let generic = original::GenericFiles::default().read_observed(original::CONFIGURED);
    assert!(
        generic.source().archive_input_origin().is_none(),
        "a generic provider cannot acquire an Archive input from an absent EntryOrigin"
    );
    Ok(())
}
