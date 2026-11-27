// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{DataForge, DataForgeError, Guid};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};

use crate::budget::Budget;
use crate::error::ToolError;

type TestResult = Result<(), Box<dyn Error>>;

const GUID: u128 = 0x1234_5678_abcd_ef01_2345_6789_abcd_ef01;
const GUID_TEXT: &str = "12345678-abcd-ef01-2345-6789abcdef01";
const INVALID_OFFSET: u32 = u32::MAX - 2;
const NAME: &str = "EntityClassDefinition.Header";
const PATH: &str = "synthetic/AudioHeader.xml";

#[derive(Clone, Copy)]
enum Word {
    Name,
    FileName,
}

impl Word {
    const fn words_before_guid(self) -> usize {
        match self {
            Self::Name => 4,
            Self::FileName => 3,
        }
    }
}

fn source(without_name: bool) -> Vec<u8> {
    let record = TestRecord::new(GUID, "AudioHeader", NAME, PATH);
    let record = if without_name {
        record.without_name()
    } else {
        record
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("AudioHeader"))
        .record(record)
        .build()
}

fn malformed(word: Word) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = source(false);
    let stored = GUID_TEXT.parse::<Guid>()?.stored();
    let mut positions = bytes
        .windows(stored.len())
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == stored).then_some(index));
    let guid = positions.next().ok_or("original synthetic Record GUID")?;
    assert!(positions.next().is_none(), "the original GUID occurs once");
    let start = guid
        .checked_sub(word.words_before_guid() * size_of::<u32>())
        .ok_or("v8 metadata word")?;
    bytes
        .get_mut(start..start + size_of::<u32>())
        .ok_or("original v8 metadata word")?
        .copy_from_slice(&INVALID_OFFSET.to_le_bytes());
    Ok(bytes)
}

fn check_error(word: Word, reference: bool) -> TestResult {
    let bytes = malformed(word)?;
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual original Record")?;
    assert_eq!(record.guid().to_string(), GUID_TEXT, "original identity");
    let original = match word {
        Word::Name => record.name(),
        Word::FileName => record.record_path(),
    };
    assert!(
        matches!(
            original,
            Err(DataForgeError::InvalidString {
                offset: INVALID_OFFSET,
                ..
            })
        ),
        "the actual original getter fails before the producer assertion"
    );
    let mut budget = Budget::new();
    let result = if reference {
        super::reference(&record, &mut budget).map(|_| ())
    } else {
        super::full(&record, &mut budget).map(|_| ())
    };
    assert!(
        matches!(
            result,
            Err(ToolError::DataForge(DataForgeError::InvalidString {
                offset: INVALID_OFFSET,
                ..
            }))
        ),
        "the exact DataForge error must not become an empty identity"
    );
    Ok(())
}

#[test]
fn audio_header_full_keeps_original_name_error() -> TestResult {
    check_error(Word::Name, false)
}

#[test]
fn audio_header_reference_keeps_original_name_error() -> TestResult {
    check_error(Word::Name, true)
}

#[test]
fn audio_header_full_keeps_original_path_error() -> TestResult {
    check_error(Word::FileName, false)
}

#[test]
fn audio_header_reference_keeps_original_path_error() -> TestResult {
    check_error(Word::FileName, true)
}

#[test]
fn audio_header_preserves_readable_and_no_name_values() -> TestResult {
    for without_name in [false, true] {
        let bytes = source(without_name);
        let forge = DataForge::open(&bytes)?;
        let record = forge.record(0).ok_or("actual original Record")?;
        let name = record.name()?;
        let path = record.record_path()?;
        assert_eq!(name, if without_name { "" } else { NAME }, "original name");
        assert_eq!(path, "synthetic/AudioHeader", "original path");
        let full = super::full(&record, &mut Budget::new())?;
        let reference = super::reference(&record, &mut Budget::new())?;
        assert_eq!(full.guid, GUID_TEXT, "complete identity");
        assert_eq!(reference.guid, full.guid, "same original GUID");
        assert_eq!(full.name, name, "no invented name");
        assert_eq!(full.record_path, path, "original normalized path");
        assert_eq!(reference.record_path, path, "same reference path");
        let class = if without_name { "" } else { "Header" };
        assert_eq!(full.class_name, class, "actual prefix conversion");
        assert_eq!(reference.class_name, class, "same reference conversion");
    }
    Ok(())
}
