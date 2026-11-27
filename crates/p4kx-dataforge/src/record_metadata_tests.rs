// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ptr;

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};

use crate::{DataForge, DataForgeError, RecordText};

type TestResult = Result<(), Box<dyn Error>>;
const GUID: u128 = 0x2222_3333_4444_5555_6666_7777_8888_9999;
const INVALID_OFFSET: u32 = u32::MAX - 1;

fn source(version: u32, row: TestRecord) -> Vec<u8> {
    DataForgeBuilder::new()
        .version(version)
        .structure(TestStruct::new("MetadataProbe"))
        .record(row)
        .build()
}

fn row() -> TestRecord {
    TestRecord::new(
        GUID,
        "MetadataProbe",
        "MetadataProbe.Name",
        "Synthetic\\Raw.XML",
    )
}

#[test]
fn original_record_metadata_getters_preserve_tag_layout_sentinel_and_empty_text() -> TestResult {
    for (version, row, expected) in [
        (6, row().tag("Domain"), RecordText::Absent),
        (8, row().without_tag(), RecordText::Null),
        (8, row().tag(""), RecordText::Present("")),
        (8, row().tag("Domain"), RecordText::Present("Domain")),
    ] {
        let bytes = source(version, row);
        let forge = DataForge::open(&bytes)?;
        let record = forge.record(0).ok_or("original Record")?;
        assert_eq!(
            record.stored_tag()?,
            expected,
            "the raw getter preserves the physical declaration state"
        );
        let tree = record.tree()?;
        assert_eq!(
            tree.metadata().metadata_tag(),
            expected,
            "the successful tree retains the same original state"
        );
        assert_eq!(
            tree.tag(),
            match expected {
                RecordText::Present(value) if !value.is_empty() => Some(value),
                _ => None,
            },
            "the old tree tag filter stays independent"
        );
    }
    Ok(())
}

#[test]
fn original_record_name_preserves_null_and_present_empty_without_changing_legacy_name() -> TestResult
{
    for (row, expected) in [
        (row().without_name(), RecordText::Null),
        (
            TestRecord::new(GUID, "MetadataProbe", "", "Synthetic\\Raw.XML"),
            RecordText::Present(""),
        ),
    ] {
        let bytes = source(8, row);
        let forge = DataForge::open(&bytes)?;
        let record = forge.record(0).ok_or("original Record")?;
        assert_eq!(
            record.stored_name()?,
            expected,
            "raw no-string and stored empty names are independent facts"
        );
        assert_eq!(
            record.name()?,
            "",
            "the old name accessor retains its contract"
        );
        let tree = record.tree()?;
        assert_eq!(
            tree.name(),
            "",
            "legacy tree consumers retain their existing empty value"
        );
        assert_eq!(
            tree.metadata().name(),
            expected,
            "the new snapshot keeps the raw declaration"
        );
    }
    Ok(())
}

#[test]
fn original_filename_snapshot_borrows_exact_text_and_leaves_normalized_path_unchanged() -> TestResult
{
    for path in [
        "Synthetic\\Raw.XML",
        "Synthetic/Mixed.xml",
        "Synthetic/NoExtension",
        "Synthetic/Name.xml.extra",
    ] {
        let bytes = source(
            8,
            TestRecord::new(GUID, "MetadataProbe", "MetadataProbe.Name", path),
        );
        let forge = DataForge::open(&bytes)?;
        let record = forge.record(0).ok_or("original Record")?;
        let tree = record.tree()?;
        assert_eq!(
            record.stored_file_name()?,
            path,
            "the original accessor does not normalize text"
        );
        assert_eq!(
            tree.metadata().file_name(),
            path,
            "the snapshot is not reconstructed by appending an extension"
        );
        assert!(
            ptr::eq(
                tree.metadata().file_name().as_ptr(),
                record.stored_file_name()?.as_ptr()
            ),
            "both views borrow the original string table"
        );
        assert_eq!(
            tree.record_path(),
            record.record_path()?,
            "the existing normalized path remains identical"
        );
    }
    Ok(())
}

#[test]
fn invalid_original_metadata_offsets_stay_errors_instead_of_becoming_absence() -> TestResult {
    let bytes = source(8, row());
    let forge = DataForge::open(&bytes)?;
    let mut record = forge.record(0).ok_or("original Record")?;
    record.def.name = INVALID_OFFSET;
    assert!(
        matches!(
            record.stored_name(),
            Err(DataForgeError::InvalidString {
                table: "blob",
                offset: INVALID_OFFSET
            })
        ),
        "a bad name offset remains unreadable"
    );
    record.def.tag = Some(INVALID_OFFSET);
    assert!(
        matches!(
            record.stored_tag(),
            Err(DataForgeError::InvalidString {
                table: "blob",
                offset: INVALID_OFFSET
            })
        ),
        "a bad tag offset is not a v6 absence or no-string sentinel"
    );
    record.def.file_name = INVALID_OFFSET;
    assert!(
        matches!(
            record.stored_file_name(),
            Err(DataForgeError::InvalidString {
                table: "text",
                offset: INVALID_OFFSET
            })
        ),
        "a bad filename offset remains the original typed error"
    );
    record.def.file_name = u32::MAX;
    assert!(
        matches!(
            record.stored_file_name(),
            Err(DataForgeError::InvalidString {
                table: "text",
                offset: u32::MAX
            })
        ),
        "the filename field has no invented no-string sentinel semantics"
    );
    Ok(())
}

#[test]
fn record_text_fold_dispatches_one_actual_state_and_preserves_empty_payloads() {
    for (value, expected) in [
        (RecordText::Absent, "absent"),
        (RecordText::Null, "null"),
        (RecordText::Present(""), ""),
    ] {
        assert_eq!(
            value.fold(|| "absent", || "null", |text| text),
            expected,
            "callers convert the complete physical state without a lossy wildcard"
        );
    }
}
