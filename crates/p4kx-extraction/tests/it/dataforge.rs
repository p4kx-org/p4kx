// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reading the DataForge Entry of an Archive.

use std::error::Error;
use std::path::Path;

use p4kx_extraction::{
    Archive, DataForgeEntry, DataForgeLoadError, DataForgeReadOptions, DataForgeSource,
};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct};

type TestResult = Result<(), Box<dyn Error>>;

fn database(records: u128) -> Vec<u8> {
    (0..records)
        .fold(
            DataForgeBuilder::new().structure(TestStruct::new("Item")),
            |builder, guid| {
                builder.record(TestRecord::new(
                    guid + 1,
                    "Item",
                    format!("Item.N{guid}"),
                    "libs/foundry/records/items.xml",
                ))
            },
        )
        .build()
}

fn archive(dir: &Path, entries: Vec<TestEntry>) -> Result<Archive, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(Archive::open_with_key(
        path,
        Some(p4kx_testkit::test_key()),
    )?)
}

#[test]
fn prefers_game2_over_game() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(
        dir.path(),
        vec![
            TestEntry::stored("Data\\Game.dcb", database(1)),
            TestEntry::zstd("Data\\Game2.dcb", database(2)).encrypted(),
        ],
    )?;

    let entry = DataForgeEntry::read(&archive)?;

    assert_eq!(entry.path(), "Data\\Game2.dcb", "Game2.dcb is preferred");
    assert_eq!(entry.open()?.record_count(), 2, "its Records are read");
    Ok(())
}

#[test]
fn falls_back_to_game_dcb() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(
        dir.path(),
        vec![
            TestEntry::stored("Data\\readme.txt", b"hello".to_vec()),
            TestEntry::deflated("data/GAME.DCB", database(3)),
        ],
    )?;

    let entry = DataForgeEntry::read(&archive)?;

    assert_eq!(
        entry.path(),
        "data/GAME.DCB",
        "case and separator do not matter"
    );
    assert_eq!(entry.open()?.record_count(), 3, "its Records are read");
    Ok(())
}

#[test]
fn locating_finds_the_entry_read_without_reading_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    let both = archive(
        dir.path(),
        vec![
            TestEntry::stored("Data\\Game.dcb", database(1)),
            TestEntry::stored("Data\\Game2.dcb", vec![0xFF; 4]),
        ],
    )?;
    let other = tempfile::tempdir()?;
    let none = archive(
        other.path(),
        vec![TestEntry::stored("Data\\Game3.dcb", database(1))],
    )?;

    let found = DataForgeEntry::locate(&both).map(p4kx_extraction::Entry::path);

    assert_eq!(found, Some("Data\\Game2.dcb"), "Game2.dcb, unread");
    assert!(DataForgeEntry::locate(&none).is_none(), "no DataForge");
    Ok(())
}

#[test]
fn a_read_stops_once_the_stop_function_says_so() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(
        dir.path(),
        vec![TestEntry::deflated("Data\\Game2.dcb", database(5))],
    )?;

    let stopped = DataForgeEntry::read_until(&archive, &|| true);
    let running = DataForgeEntry::read_until(&archive, &|| false)?;

    assert!(
        matches!(stopped, Err(DataForgeLoadError::Cancelled)),
        "stopped: {stopped:?}"
    );
    assert_eq!(running.open()?.record_count(), 5, "read to the end");
    Ok(())
}

#[test]
fn an_archive_without_dataforge_is_an_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(
        dir.path(),
        vec![TestEntry::stored("Data\\Game3.dcb", database(1))],
    )?;

    let result = DataForgeEntry::read(&archive);

    assert!(
        matches!(result, Err(DataForgeLoadError::Missing)),
        "no DataForge Entry: {result:?}"
    );
    Ok(())
}

#[test]
fn corrupt_entry_data_is_a_read_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(
        dir.path(),
        vec![TestEntry::deflated("Data\\Game2.dcb", database(1)).with_payload(vec![0xFF; 8])],
    )?;

    let result = DataForgeEntry::read(&archive);

    assert!(
        matches!(result, Err(DataForgeLoadError::Read { .. })),
        "the Entry cannot be decompressed: {result:?}"
    );
    Ok(())
}

#[test]
fn an_explicit_byte_budget_refuses_the_archive_before_decoding() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(
        dir.path(),
        vec![
            TestEntry::deflated("Data\\Game2.dcb", database(1))
                .with_payload(vec![0xFF; 8])
                .with_uncompressed_size(17),
        ],
    )?;
    let options = DataForgeReadOptions::default().with_max_decoded_bytes(16);

    let result = DataForgeSource::Archive(&archive).read_with_options(options, &|| false);

    assert!(
        matches!(
            result,
            Err(DataForgeLoadError::BudgetExceeded {
                size: 17,
                limit: 16,
                ..
            })
        ),
        "the caller's budget precedes reading the damaged payload: {result:?}"
    );
    Ok(())
}

#[test]
fn a_platform_unaddressable_declaration_is_a_capacity_error() -> TestResult {
    let limit = isize::MAX as u64;
    let dir = tempfile::tempdir()?;
    let archive = archive(
        dir.path(),
        vec![TestEntry::stored("Data\\Game2.dcb", database(1)).with_uncompressed_size(limit + 1)],
    )?;

    let result = DataForgeEntry::read(&archive);

    assert!(
        matches!(result, Err(DataForgeLoadError::TooLarge { size, limit: actual, .. })
            if size == limit + 1 && actual == limit),
        "representability is checked without allocating: {result:?}"
    );
    Ok(())
}

#[test]
fn a_decoded_tail_beyond_the_declared_dataforge_is_a_read_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let original = database(1);
    let short_declaration = u64::try_from(original.len())? - 1;
    let archive = archive(
        dir.path(),
        vec![
            TestEntry::stored("Data\\Game2.dcb", original)
                .with_uncompressed_size(short_declaration),
        ],
    )?;

    let result = DataForgeEntry::read(&archive);

    assert!(
        matches!(result, Err(DataForgeLoadError::Read { .. })),
        "the decoder must prove EOF at the declared length: {result:?}"
    );
    Ok(())
}

#[cfg(target_pointer_width = "64")]
#[test]
fn a_declaration_above_two_gibibytes_checks_the_actual_end_of_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(
        dir.path(),
        vec![
            TestEntry::stored("Data\\Game2.dcb", database(1)).with_uncompressed_size((2 << 30) + 1),
        ],
    )?;

    let result = DataForgeEntry::read(&archive);

    assert!(
        matches!(result, Err(DataForgeLoadError::Read { .. })),
        "the Entry ends before its claimed size: {result:?}"
    );
    Ok(())
}
