// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{AudioIndex, SoundbankEventSource};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestHircObject};

use super::TestResult;

const EVENT: u32 = 4242;
const VERSION: u32 = 154;

#[test]
fn event_sources_retain_physical_aliases_and_distinct_object_occurrences() -> TestResult {
    const PATH: &str = "Data\\Sounds\\wwise\\Twin.bnk";
    let bytes = [
        TestBank::new(1)
            .version(VERSION)
            .object(TestHircObject::event(5, &[]))
            .object(TestHircObject::event(EVENT, &[]))
            .object(TestHircObject::event(EVENT, &[]))
            .bytes(),
        TestBank::new(2)
            .version(VERSION)
            .object(TestHircObject::event(EVENT, &[]))
            .bytes(),
    ];
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            PATH,
            bytes.first().ok_or("first bank")?.clone(),
        ))
        .entry(TestEntry::stored(
            PATH,
            bytes.get(1).ok_or("second bank")?.clone(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let index = AudioIndex::build(&archive);
    assert_eq!(
        index.event_soundbanks(EVENT).count(),
        2,
        "one existing holder per physical bank"
    );
    let mut positions = Vec::new();
    for source in index.event_soundbank_sources(&archive, EVENT) {
        let position = source.soundbank().entry_position();
        assert_eq!(source.soundbank().path(), PATH, "same path in both banks");
        assert!(
            archive
                .entries()
                .get(position)
                .is_some_and(|entry| std::ptr::eq(entry, source.entry().entry())),
            "the actual owning Entry is bound"
        );
        let bank = bytes.get(position).ok_or("actual test bank")?;
        assert_original_bytes(bank, &source)?;
        positions.push((position, source.object_ordinal()));
    }
    assert_eq!(
        positions,
        [(0, 1), (0, 2), (1, 0)],
        "source order and exact occurrences"
    );

    let reopened = Archive::open(&path)?;
    assert_eq!(
        index.event_soundbank_sources(&reopened, EVENT).count(),
        0,
        "equal bytes in a separately admitted Archive cannot replace the original owner"
    );
    Ok(())
}

fn assert_original_bytes(bank: &[u8], source: &SoundbankEventSource<'_, '_>) -> TestResult {
    let version = source
        .version_bytes()
        .ok_or("original version declaration")?;
    assert_eq!(
        bank.get(version),
        Some(VERSION.to_le_bytes().as_slice()),
        "bank version selector"
    );
    assert_eq!(
        bank.get(source.kind_bytes()),
        Some([4_u8].as_slice()),
        "HIRC Event kind byte"
    );
    assert_eq!(
        bank.get(source.id_bytes()),
        Some(EVENT.to_le_bytes().as_slice()),
        "HIRC Event ID"
    );
    Ok(())
}
