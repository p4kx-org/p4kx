// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, SourceInventory, SourceLocation, SourceUseRole};
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestContainer, TestEntry};

use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::SourceDisposition;

#[test]
fn a_failed_soc_does_not_hide_later_selected_sources() -> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let container = TestContainer::new()
        .entry(TestEntry::stored("bad.soc", "invalid chunk file"))
        .entry(TestEntry::stored(
            "valid.soc",
            TestChunkFile::crch().bytes(),
        ));
    ArchiveBuilder::new()
        .entry(TestEntry::stored("loot.socpak", container.build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;

    let (counts, ledger) =
        crate::catalog_read::original::with_entry_capture(&archive, |capture| {
            super::read(&archive, 0, None, capture)
        })?;

    assert!(
        counts.tags.is_err(),
        "a later success cannot erase an earlier failure"
    );
    let [outer, failed, valid] = ledger.entries.as_slice() else {
        return Err("every selected source must be observed".into());
    };
    assert_eq!(
        outer.operation,
        EntryOperation::ContainerDirectory,
        "actual outer operation"
    );
    assert_eq!(
        failed.disposition,
        SourceDisposition::ProjectionFailed,
        "decoded bytes are not a SOC"
    );
    assert_eq!(
        valid.disposition,
        SourceDisposition::Projected,
        "valid zero-entity SOC succeeds"
    );
    assert_eq!(
        valid.operation,
        EntryOperation::SocTags,
        "actual SOC count operation"
    );
    Ok(())
}

#[test]
fn an_opened_container_without_soc_members_proves_zero_from_the_original_directory()
-> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let container = TestContainer::new().entry(TestEntry::stored(
        "other.txt",
        "payload with no entity role",
    ));
    ArchiveBuilder::new()
        .entry(TestEntry::stored("loot.socpak", container.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let (counted, inventory) =
        crate::catalog_read::original::with_entry_capture(&archive, |capture| {
            let (counted, _) = super::read(&archive, 0, None, capture)?;
            Ok((counted, capture.source_uses().freeze()?))
        })?;
    assert_eq!(
        counted.tags?.entities, 0,
        "the complete directory contains no admitted SOC entries"
    );
    assert!(
        counted.origin.source_set_complete(),
        "the nested directory proves the zero"
    );
    assert_eq!(
        counted.origin.contains_encrypted_source(),
        Some(true),
        "the nested directory came from the encrypted original container payload",
    );
    let [reference] = counted.origin.source_uses_refs() else {
        return Err("zero requires only the original nested directory survey".into());
    };
    let directory = inventory
        .iter_uses()
        .find(|used| used.id() == reference)
        .ok_or("physical nested directory contribution")?;
    assert!(
        matches!(
            directory.location(),
            SourceLocation::Subresource {namespace, field, indices}
                if namespace == "socArchiveDirectory" && field == "entryPaths" && indices == &[0, 1]
        ) && directory.role() == SourceUseRole::Selector,
        "the consulted nested path range supplies the membership decision"
    );
    Ok(())
}

#[test]
fn a_bad_soc_crc_retains_an_attempt_and_never_supplies_entity_payload()
-> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let chunk = |id| {
        TestChunkFile::crch()
            .chunk(0x0004, id, b"<Entities><Entity/></Entities>".to_vec())
            .bytes()
    };
    let container = TestContainer::new()
        .entry(TestEntry::stored("bad.soc", chunk(1)).with_crc32(0))
        .entry(TestEntry::stored("good.soc", chunk(2)))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("loot.socpak", container))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let (counted, ledger, inventory) =
        crate::catalog_read::original::with_entry_capture(&archive, |capture| {
            let (counted, ledger) = super::read(&archive, 0, None, capture)?;
            Ok((counted, ledger, capture.source_uses().freeze()?))
        })?;
    assert_eq!(
        counted.tags,
        Err(crate::loot_unreadable::Unreadable::SocFile),
        "a selected member with parseable bytes but a wrong CRC cannot contribute",
    );
    assert!(
        !counted.origin.source_set_complete(),
        "the valid neighboring SOC cannot establish a complete entity count",
    );
    assert!(
        ledger
            .entries
            .iter()
            .any(|entry| entry.operation == EntryOperation::SocTags
                && entry.disposition == SourceDisposition::ReadFailed),
        "the bad checksum is a required read failure",
    );
    let uses = counted.origin.source_uses_refs();
    let payloads: Vec<_> = inventory
        .iter_uses()
        .filter(|used| uses.contains(used.id()) && used.role() == SourceUseRole::Payload)
        .collect();
    let [payload] = payloads.as_slice() else {
        return Err("only one verified selected entity chunk may contribute".into());
    };
    assert!(
        matches!(payload.location(), SourceLocation::Subresource {namespace, ..}
            if namespace == "socEntityChunk"),
        "the surviving evidence addresses only the selected original XML chunk",
    );
    assert_failed_attempt(&inventory, uses, payload.source());
    Ok(())
}

fn assert_failed_attempt(
    inventory: &SourceInventory,
    count_uses: &[p4kx_extraction::SourceUseRef],
    accepted: &p4kx_extraction::PhysicalSourceRef,
) {
    let failed = inventory
        .iter_uses()
        .filter(|used| used.role() == SourceUseRole::Attempted)
        .filter(|used| used.scope() == p4kx_extraction::InputScope::Payload)
        .filter(|used| used.source() != accepted);
    let absent = failed
        .filter(|used| !count_uses.contains(used.id()))
        .count();
    assert_eq!(
        absent, 2,
        "outer and failed SOC attempts stay distinct from contributors"
    );
}
