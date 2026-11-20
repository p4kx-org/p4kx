// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, ArchiveLayoutKind, ArchiveSummary, EntryIssue, FieldPath, InputScope, MetadataRange,
    ObservedRecoveryDataOutsideFile, ObservedRecoveryZip64Selection, SourceLocation,
    SourceUseDescription, SourceUseRole,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{ENTRY, TestResult, open, put_word};

const V1_CENTRAL: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];
const V1_ZIP64_END: [u8; 4] = [0x50, 0x4b, 0x06, 0x06];
const ZIP_CENTRAL_FIXED_BYTES: usize = 46;
const ZIP64_EXTRA_COMPRESSED_WORD: usize = 12;
const UNRELATED_BLOCK: [u8; 9] = [0xEF, 0xBE, 9, 0, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9];

#[test]
fn recovery_data_extent_binds_the_selected_zip_words_and_signature() -> TestResult {
    for zip64 in [false, true] {
        let (bytes, boundary, selected_size) = recovery_extent_fixture(zip64)?;
        let (_directory, archive) = open(&bytes, true)?;
        let (position, bounds) = recovery_issue(&archive)?;
        assert_eq!(
            archive.metadata().layout(),
            ArchiveLayoutKind::RecoveredUnknown,
            "recovery route"
        );
        assert!(
            archive.metadata().zip_directory().is_none(),
            "ZIP trailer is unknown"
        );
        assert!(
            archive.metadata().v2_directory().is_none(),
            "v2 trailer is unknown"
        );
        assert!(
            archive.observed_v2_data_outside_file(position).is_none(),
            "no v2 extent"
        );
        assert!(
            archive.observed_recovery_data_outside_file(0).is_none(),
            "archive-wide issue has no operands"
        );
        assert!(
            boundary + 1 < bytes.len(),
            "computed end crosses the candidate inside the file"
        );
        let winning = zip64.then(|| (selected_size - ZIP64_EXTRA_COMPRESSED_WORD) as u64);
        check_recovery_bounds(bounds, boundary as u64, selected_size as u64, winning);
        check_recovery_origins(&archive, position, winning)?;
    }
    Ok(())
}

fn recovery_issue(archive: &Archive) -> TestResult<(usize, ObservedRecoveryDataOutsideFile)> {
    let position = archive
        .issues()
        .iter()
        .position(|(_, issue)| *issue == EntryIssue::DataOutsideFile)
        .ok_or("ZIP recovery data extent defect")?;
    let bounds = archive
        .observed_recovery_data_outside_file(position)
        .ok_or("observed recovery operands")?;
    Ok((position, bounds))
}

fn check_recovery_bounds(
    bounds: ObservedRecoveryDataOutsideFile,
    boundary: u64,
    selected_size: u64,
    winning: Option<u64>,
) {
    let word_width = if winning.is_some() { 8 } else { 4 };
    assert_eq!(
        bounds.candidate_signature().offset(),
        boundary,
        "observed boundary"
    );
    assert_eq!(
        bounds.candidate_signature().len(),
        4,
        "central signature length"
    );
    check_local_header(bounds, word_width);
    assert_eq!(
        bounds.data_start() + bounds.compressed_size().value(),
        boundary + 1,
        "observed comparison"
    );
    assert_eq!(
        bounds.compressed_size().range().offset(),
        selected_size,
        "selected compressed-size word"
    );
    assert_eq!(
        bounds.compressed_size().range().len(),
        word_width,
        "size word length"
    );
    let zip64 = bounds.zip64_selection();
    assert_eq!(
        zip64.is_some(),
        winning.is_some(),
        "extent selects ZIP64 words"
    );
    assert_eq!(
        zip64
            .and_then(ObservedRecoveryZip64Selection::header)
            .map(MetadataRange::offset),
        winning,
        "winning ZIP64 block header offset",
    );
}

fn check_local_header(bounds: ObservedRecoveryDataOutsideFile, word_width: u64) {
    assert_eq!(
        bounds.local_header_offset().value(),
        0,
        "selected local header offset"
    );
    assert_eq!(
        bounds.local_header_offset().range().len(),
        word_width,
        "offset word length"
    );
    assert_eq!(
        bounds.local_name_length().value(),
        ENTRY.len() as u64,
        "stored name length"
    );
    assert_eq!(
        bounds.local_name_length().range().offset(),
        26,
        "local name length offset"
    );
    assert_eq!(
        bounds.local_extra_length().range().offset(),
        28,
        "local extra length offset"
    );
    assert_eq!(
        bounds.local_extra_length().value(),
        3,
        "stored extra length"
    );
}

fn recovery_extent_fixture(zip64: bool) -> TestResult<(Vec<u8>, usize, usize)> {
    let mut bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored(ENTRY, b"data"))
        .build();
    let local_name_end = 30 + ENTRY.len();
    write_bytes(&mut bytes, 28, &3_u16.to_le_bytes())?;
    bytes.splice(local_name_end..local_name_end, [0; 3]);
    let central = marker(&bytes, V1_CENTRAL)?;
    let selected_size = if zip64 {
        let extra = central + ZIP_CENTRAL_FIXED_BYTES + ENTRY.len();
        let block_len = read_u16(&bytes, extra + 2)?;
        let insert = extra + usize::from(block_len);
        let first_block = bytes
            .get(extra..insert)
            .ok_or("first ZIP64 extra block")?
            .to_vec();
        let size = central + 30;
        let extra_len = read_u16(&bytes, size)?;
        let added = block_len + u16::try_from(UNRELATED_BLOCK.len())?;
        write_bytes(&mut bytes, size, &(extra_len + added).to_le_bytes())?;
        bytes.splice(
            insert..insert,
            UNRELATED_BLOCK.into_iter().chain(first_block),
        );
        let at = insert + UNRELATED_BLOCK.len() + ZIP64_EXTRA_COMPRESSED_WORD;
        put_word(&mut bytes, at, 5)?;
        at
    } else {
        for (relative, value) in [(20, 5_u32), (42, 0)] {
            let at = central + relative;
            write_bytes(&mut bytes, at, &value.to_le_bytes())?;
        }
        central + 20
    };
    bytes.truncate(marker(&bytes, V1_ZIP64_END)?);
    Ok((bytes, central, selected_size))
}

fn check_recovery_origins(archive: &Archive, position: usize, winning: Option<u64>) -> TestResult {
    let (_, output) = ArchiveSummary::project_with_issue_origins(archive, "", position + 1)?;
    let origin = output
        .field_origins
        .get(&FieldPath::parse(&format!("/issues/{position}/issue"))?)
        .ok_or("recovery issue field origin")?;
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(false),
        "clear archive framing"
    );
    assert!(
        origin.source_set_complete(),
        "the local extent check inputs are known"
    );
    let uses: Vec<_> = output
        .provenance
        .iter_uses()
        .filter(|used| origin.source_uses_refs().contains(used.id()))
        .collect();
    assert!(
        uses.iter()
            .all(|used| used.scope() == InputScope::ArchiveDirectoryMetadata),
        "all operands are in the archive metadata domain",
    );
    assert!(
        uses.iter().any(|used| {
            used.role() == SourceUseRole::Selector
                && matches!(used.location(), SourceLocation::Subresource { namespace, field, .. }
                if namespace == "zipRecoveryBoundary" && field == "centralHeaderSignature")
        }),
        "the candidate is a signature position, not a declared bound word"
    );
    check_selected_header(&uses, winning);
    Ok(())
}

fn check_selected_header(uses: &[&SourceUseDescription], winning: Option<u64>) {
    let header = uses.iter().find_map(|used| match used.location() {
        SourceLocation::Subresource {
            namespace,
            indices,
            field,
        } if namespace == "zip64OperandSelection" && field == "winningHeader" => {
            assert_eq!(
                used.role(),
                SourceUseRole::Selector,
                "winning header is a selector"
            );
            Some(indices.as_slice())
        }
        _ => None,
    });
    let expected = winning.map(|offset| [offset, offset + 4]);
    assert_eq!(
        header,
        expected.as_ref().map(<[u64; 2]>::as_slice),
        "exact winning header range"
    );
    if let Some(offset) = winning {
        let unrelated = offset - UNRELATED_BLOCK.len() as u64..offset;
        for used in uses {
            if let SourceLocation::Subresource { indices, .. } = used.location()
                && let [first, end] = indices.as_slice()
            {
                assert!(
                    *end <= unrelated.start || *first >= unrelated.end,
                    "the unrelated extra header and payload supply no selected input"
                );
            }
        }
    }
}

#[test]
fn a_recovery_sentinel_without_a_winning_header_has_incomplete_origins() -> TestResult {
    let (mut bytes, boundary, selected_size) = recovery_extent_fixture(false)?;
    let fixed_extra = boundary + ZIP_CENTRAL_FIXED_BYTES + ENTRY.len();
    write_bytes(&mut bytes, fixed_extra, &0xBEEF_u16.to_le_bytes())?;
    write_bytes(&mut bytes, selected_size, &u32::MAX.to_le_bytes())?;

    let (_directory, archive) = open(&bytes, true)?;
    let (position, bounds) = recovery_issue(&archive)?;
    assert_eq!(
        bounds.compressed_size().value(),
        u64::from(u32::MAX),
        "retained fixed sentinel"
    );
    assert_eq!(
        bounds.compressed_size().range().offset(),
        selected_size as u64,
        "fixed compressed-size word position",
    );
    let selection = bounds.zip64_selection().ok_or("observed fixed selector")?;
    assert!(selection.header().is_none(), "no winning ZIP64 header");
    assert!(
        !selection.complete(),
        "nonempty extras were not selected as a whole"
    );

    let (_, output) = ArchiveSummary::project_with_issue_origins(&archive, "", position + 1)?;
    let origin = output
        .field_origins
        .get(&FieldPath::parse(&format!("/issues/{position}/issue"))?)
        .ok_or("recovery issue field origin")?;
    assert!(!origin.source_set_complete(), "no fully observed selection");
    assert_eq!(
        origin.contains_encrypted_source(),
        None,
        "unknown source marker"
    );
    assert!(
        !origin.source_uses_refs().is_empty(),
        "known original words survive"
    );
    Ok(())
}

fn read_u16(bytes: &[u8], at: usize) -> TestResult<u16> {
    let word = bytes
        .get(at..)
        .and_then(<[u8]>::first_chunk::<2>)
        .ok_or("synthetic ZIP length word")?;
    Ok(u16::from_le_bytes(*word))
}

fn write_bytes(bytes: &mut [u8], at: usize, value: &[u8]) -> TestResult {
    let field = bytes
        .get_mut(at..)
        .and_then(|rest| rest.get_mut(..value.len()))
        .ok_or("synthetic ZIP field to replace")?;
    field.copy_from_slice(value);
    Ok(())
}

fn marker(bytes: &[u8], signature: [u8; 4]) -> TestResult<usize> {
    bytes
        .windows(signature.len())
        .position(|window| window == signature)
        .ok_or_else(|| "synthetic ZIP section".into())
}
