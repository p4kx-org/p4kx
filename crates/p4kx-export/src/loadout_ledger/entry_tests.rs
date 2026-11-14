// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ArchiveFiles;
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::export_entry_ledger::EntryOperation;

use super::{
    Arc, Capture, CatalogGuid, DataForge, DataForgeBuilder, Entities, EntityKind, ExportLedger,
    Kind, LoadoutResolver, PortReader, ROOT, SourceDisposition, TestEntity, TestLoadout,
    TestResult, Wanted,
};

pub(super) const CONFIGURED: &str = "Scripts/Loadouts/Synthetic/root.xml";
const ACTUAL: &str = "Data\\Scripts\\Loadouts\\Synthetic\\root.xml";
const VEHICLE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Synthetic.xml";
const VEHICLE_ENTRY: &str =
    "Data\\Scripts\\Entities\\Vehicles\\Implementations\\Xml\\Synthetic.xml";

pub(super) fn archive(
    entries: Vec<TestEntry>,
) -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("Data.p4k");
    let mut builder = ArchiveBuilder::new();
    for entry in entries {
        builder = builder.entry(entry);
    }
    builder.write_to(&path)?;
    Ok((root, Archive::open(&path)?))
}

fn run(
    entries: Vec<TestEntry>,
    vehicle: Option<&str>,
    twice: bool,
) -> Result<ExportLedger, Box<dyn std::error::Error>> {
    let (_root, archive) = archive(entries)?;
    let files = ArchiveFiles::new(&archive);
    let entity = || TestEntity::new(ROOT, "Root", "synthetic/root.xml");
    let entity = vehicle.map_or_else(
        || entity().loadout(TestLoadout::Xml(CONFIGURED.to_owned())),
        |modification| entity().vehicle(VEHICLE, modification),
    );
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(entity.record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("source missing")?;
    let tree = record.tree()?;
    let capture = Arc::new(Capture::new(&super::source(&record, &tree)?, None));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), capture.clone())
            .with_files(&files);
    for _ in 0..=usize::from(twice) {
        crate::loadout::read_with(
            &mut resolver,
            &mut PortReader::new(),
            &forge,
            Wanted {
                record_index: record.index(),
                guid: CatalogGuid(record.guid()),
                kind: EntityKind::Item,
            },
            &capture,
        )
        .map_err(|failure| failure.reason().to_owned())?;
    }
    Ok(capture.finish()?)
}

#[test]
fn actual_xml_read_and_projection_keep_one_entry_identity_and_cache_reuse() -> TestResult {
    let ledger = run(
        vec![TestEntry::stored(ACTUAL, b"<Loadout/>".to_vec())],
        None,
        true,
    )?;
    assert_eq!(
        ledger.entries.len(),
        2,
        "cache reuse neither rereads bytes nor invents another parse"
    );
    for operation in [EntryOperation::XmlLoadoutBytes, EntryOperation::XmlLoadout] {
        let entry = ledger
            .entries
            .iter()
            .find(|entry| entry.operation == operation)
            .ok_or("operation absent")?;
        assert_eq!(
            entry.entry_index,
            Some(0),
            "the actual Archive table position is retained"
        );
        assert_eq!(
            entry.path.as_str(),
            ACTUAL,
            "canonical Entry spelling comes from the actual Archive"
        );
        assert_eq!(
            entry.disposition,
            SourceDisposition::Projected,
            "the actual operation completed"
        );
    }
    assert!(
        ledger.incomplete_inventories.is_empty(),
        "successful complete input enumeration stays complete"
    );
    Ok(())
}

#[test]
fn present_malformed_xml_fails_projection_after_a_successful_actual_read() -> TestResult {
    let ledger = run(
        vec![TestEntry::stored(ACTUAL, b"<broken>".to_vec())],
        None,
        false,
    )?;
    let bytes = ledger
        .entries
        .iter()
        .find(|entry| entry.operation == EntryOperation::XmlLoadoutBytes)
        .ok_or("read absent")?;
    let parse = ledger
        .entries
        .iter()
        .find(|entry| entry.operation == EntryOperation::XmlLoadout)
        .ok_or("parse absent")?;
    assert_eq!(
        bytes.disposition,
        SourceDisposition::Projected,
        "the existing bytes were actually read"
    );
    assert_eq!(
        parse.disposition,
        SourceDisposition::ProjectionFailed,
        "a real parser failure remains separate"
    );
    assert_eq!(
        parse.entry_index, bytes.entry_index,
        "read and parse describe the same physical file"
    );
    assert!(
        ledger.incomplete_inventories.contains(&Kind::Loadouts),
        "unread declarations may hide required nested inputs"
    );
    Ok(())
}

#[test]
fn actual_archive_read_errors_do_not_claim_a_document_parse() -> TestResult {
    let ledger = run(
        vec![TestEntry::stored(ACTUAL, b"<Loadout/>".to_vec()).encrypted()],
        None,
        false,
    )?;
    assert_eq!(
        ledger.entries.len(),
        1,
        "the unread source has no fabricated parse event"
    );
    assert_eq!(
        ledger.entries[0].operation,
        EntryOperation::XmlLoadoutBytes,
        "only an actual byte-read was attempted"
    );
    assert_eq!(
        ledger.entries[0].entry_index,
        Some(0),
        "an unreadable present file keeps its actual identity"
    );
    assert_eq!(
        ledger.entries[0].disposition,
        SourceDisposition::ReadFailed,
        "required byte access failed"
    );
    assert!(
        ledger.incomplete_inventories.contains(&Kind::Loadouts),
        "unread input enumeration stays incomplete"
    );
    Ok(())
}

#[test]
fn configured_missing_xml_is_known_absent_and_does_not_invent_an_incomplete_inventory() -> TestResult
{
    let ledger = run(
        vec![TestEntry::stored(
            "Data\\unrelated.xml",
            b"<Loadout/>".to_vec(),
        )],
        None,
        false,
    )?;
    assert_eq!(
        ledger.entries.len(),
        1,
        "the configured missing target is represented once"
    );
    assert_eq!(
        ledger.entries[0].entry_index, None,
        "no Archive index is fabricated"
    );
    assert_eq!(
        ledger.entries[0].path.as_str(),
        CONFIGURED,
        "the configured target remains available"
    );
    assert_eq!(
        ledger.entries[0].disposition,
        SourceDisposition::SkippedBeforeRead,
        "known absence is intentional representation"
    );
    assert!(
        ledger.incomplete_inventories.is_empty(),
        "a known absent target does not imply unknown traversal"
    );
    Ok(())
}

#[test]
fn vehicle_base_and_patch_parse_are_distinct_actual_document_operations() -> TestResult {
    let base = br#"<Vehicle><Modifications><Modification name="Fancy" patchFile="Modifications/explicit.xml"/></Modifications></Vehicle>"#;
    let patch =
        "Data\\Scripts\\Entities\\Vehicles\\Implementations\\Xml\\Modifications\\explicit.xml";
    let ledger = run(
        vec![
            TestEntry::stored(VEHICLE_ENTRY, base.to_vec()),
            TestEntry::stored(patch, b"<Patch/>".to_vec()),
        ],
        Some("Fancy"),
        true,
    )?;
    for (operation, index) in [
        (EntryOperation::VehicleImplementation, 0),
        (EntryOperation::VehiclePatch, 1),
    ] {
        let entry = ledger
            .entries
            .iter()
            .find(|entry| entry.operation == operation)
            .ok_or("actual parse operation absent")?;
        assert_eq!(
            entry.entry_index,
            Some(index),
            "the actual file supplies its own projection identity"
        );
        assert_eq!(
            entry.disposition,
            SourceDisposition::Projected,
            "the invoked intrinsic parse completed"
        );
    }
    assert_eq!(
        ledger
            .entries
            .iter()
            .filter(|entry| matches!(
                entry.operation,
                EntryOperation::VehicleImplementationBytes | EntryOperation::VehiclePatchBytes
            ))
            .count(),
        2,
        "the base and patch bytes are each read once across cache reuse"
    );
    Ok(())
}

#[test]
fn xml_include_depth_marks_unknown_remaining_inputs_without_fictitious_reads() -> TestResult {
    let mut entries = Vec::new();
    for depth in 0..=p4kx_assembly::MAX_XML_INCLUDE_DEPTH {
        let configured = if depth == 0 {
            CONFIGURED.to_owned()
        } else {
            format!("Scripts/Loadouts/Synthetic/{depth}.xml")
        };
        let next = format!("Scripts/Loadouts/Synthetic/{}.xml", depth + 1);
        let contents =
            format!("<Loadout><Loadouts><Loadout loadout=\"{next}\"/></Loadouts></Loadout>");
        entries.push(TestEntry::stored(
            format!("Data/{configured}"),
            contents.into_bytes(),
        ));
    }
    let ledger = run(entries, None, false)?;
    assert!(
        ledger.incomplete_inventories.contains(&Kind::Loadouts),
        "the actual include traversal limit remains explicit"
    );
    assert_eq!(
        ledger
            .entries
            .iter()
            .filter(|entry| entry.operation == EntryOperation::XmlLoadoutBytes)
            .count(),
        p4kx_assembly::MAX_XML_INCLUDE_DEPTH,
        "files beyond the stopped traversal are not reported as read"
    );
    assert!(
        ledger
            .entries
            .iter()
            .all(|entry| entry.disposition == SourceDisposition::Projected),
        "successfully read and parsed ancestors are retained independently of the traversal limit"
    );
    Ok(())
}
