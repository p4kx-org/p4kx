// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{Cursor, Read as _};
use std::path::Path;

use p4kx_extraction::bank::BankSource;
use p4kx_extraction::{Archive, ByteOrder, MediaClass, OpenOptions, SectionId, Wem};
use p4kx_media::{ExportOptions, ExportOutcome, Identity, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, test_key};
use serde_json::{Value, json};

use super::fixture::{self, Fixture};
use super::origins::{self, Origins};
use super::{AUDIO_STREAM, TestResult, check_schema, options};
use crate::support;

const PATH: &str = "embedded.Wwise.BNK";
const PREFETCH: &[u8] = b"RIFF\x50\0\0\0WAVE";
const DIDX_SECTION: u64 = 1;
const MEDIA_ID: u32 = 49;
const TRAILING: [u8; 2] = [0xf1, 0xf2];

#[test]
fn embedded_wem_occurrences_keep_exact_banked_ranges_and_origins() -> TestResult {
    let folder = tempfile::tempdir()?;
    let fixtures = [
        fixture::fixture(ByteOrder::Little)?,
        fixture::fixture(ByteOrder::Big)?,
    ];
    let second = [fixtures[1].bytes.as_slice(), TRAILING.as_slice()].concat();
    let bytes = TestBank::new(7)
        .wem(MEDIA_ID, fixtures[0].bytes.as_slice())
        .wem(MEDIA_ID, second)
        .wem(MEDIA_ID, PREFETCH)
        .wem(50, b"PLUGopaque".as_slice())
        .bytes();
    let archive = duplicate_archive(folder.path(), &bytes)?;
    let starts = original_starts(&archive, &bytes, &fixtures)?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let directory = output.join(&index.artifact_base);
    let rows = support::records(&output, &index, AUDIO_STREAM)?;
    check_schema(&rows)?;
    let origins = Origins::read(&directory)?;
    for (owner, starts) in starts.into_iter().enumerate() {
        let records = owned_rows(&rows, owner);
        check_owner(
            &directory, &bytes, &records, &fixtures, starts, owner, &origins,
        )?;
    }
    Ok(())
}

fn duplicate_archive(directory: &Path, bytes: &[u8]) -> TestResult<Archive> {
    let path = directory.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(PATH, bytes))
        .entry(TestEntry::zstd(PATH, bytes).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn original_starts(
    archive: &Archive,
    expected: &[u8],
    fixtures: &[Fixture; 2],
) -> TestResult<[[usize; 2]; 2]> {
    let [plain, encrypted] = archive.entries() else {
        return Err("two physical original bank Entries".into());
    };
    assert!(
        !plain.is_encrypted() && encrypted.is_encrypted(),
        "opposite payload markers"
    );
    let mut starts = [[0; 2]; 2];
    for (entry, targets) in [plain, encrypted].into_iter().zip(&mut starts) {
        let mut bytes = Vec::new();
        archive
            .open_entry_verified(entry)?
            .reject_overlong()
            .read_to_end(&mut bytes)?;
        assert_eq!(
            bytes, expected,
            "actual original decoded banks have equal bytes"
        );
        *targets = locate_media(&bytes, fixtures)?;
    }
    Ok(starts)
}

fn locate_media(bytes: &[u8], fixtures: &[Fixture; 2]) -> TestResult<[usize; 2]> {
    let mut bank = BankSource::open(Cursor::new(bytes), u64::try_from(bytes.len())?)?;
    let sections = bank.sections().collect::<Result<Vec<_>, _>>()?;
    let [_, index, data] = sections.as_slice() else {
        return Err("one original DIDX and DATA".into());
    };
    assert_eq!(
        (index.ordinal(), data.id()),
        (DIDX_SECTION, SectionId::DATA),
        "source section route"
    );
    let mut cursor = bank.didx(index)?;
    let mut starts = [0; 2];
    for (slot, (fixture, start)) in fixtures.iter().zip(&mut starts).enumerate() {
        let occurrence = cursor.next().ok_or("actual complete DIDX occurrence")??;
        let media = cursor.inspect_media(&occurrence, data)?;
        assert_eq!(occurrence.media_id(), MEDIA_ID, "repeated actual short ID");
        assert_eq!(
            (media.class, media.prefetch),
            (MediaClass::Wem, false),
            "full actual WEM"
        );
        *start = usize::try_from(media.range.start)?;
        let raw = bytes
            .get(*start..usize::try_from(media.range.end)?)
            .ok_or("original admitted DATA range")?;
        let trailing = if slot == 1 { TRAILING.as_slice() } else { &[] };
        assert_eq!(
            raw,
            [fixture.bytes.as_slice(), trailing].concat(),
            "original WEM plus exact optional tail"
        );
        assert_eq!(
            Wem::parse(raw)?.sample_count(),
            Some(2),
            "actual WEM metadata independently reads"
        );
    }
    let prefetch = cursor.next().ok_or("actual prefetch triplet")??;
    assert_eq!(
        prefetch.media_id(),
        MEDIA_ID,
        "prefetch deliberately repeats the ID"
    );
    assert!(
        cursor.inspect_media(&prefetch, data)?.prefetch,
        "actual retained prefix"
    );
    let plugin = cursor.next().ok_or("actual plug-in triplet")??;
    assert_eq!(
        cursor.inspect_media(&plugin, data)?.class,
        MediaClass::Impulse,
        "actual opaque payload"
    );
    assert!(cursor.next().is_none(), "no other original media");
    Ok(starts)
}

fn owned_rows(rows: &[MediaRecord], owner: usize) -> Vec<(usize, &MediaRecord)> {
    rows.iter()
        .enumerate()
        .filter(|(_, record)| {
            matches!(&record.source.identity, Identity::Entry { entry, .. } if entry.entry_index == owner)
        })
        .collect()
}

fn check_owner(
    directory: &Path,
    bytes: &[u8],
    records: &[(usize, &MediaRecord)],
    fixtures: &[Fixture; 2],
    starts: [usize; 2],
    owner: usize,
    origins: &Origins,
) -> TestResult {
    let raw = records
        .first()
        .and_then(|(_, row)| row.source.raw.as_ref())
        .ok_or("exact retained bank payload")?;
    assert_eq!(
        fs::read(directory.join(&raw.path))?,
        bytes,
        "raw source is the entire bank"
    );
    assert!(
        records
            .iter()
            .filter(|(_, row)| row.subresource.namespace.starts_with("audio-bank-wem"))
            .all(|(_, row)| matches!(
                row.subresource.indices.as_slice(),
                [DIDX_SECTION, 0 | 1, ..]
            )),
        "prefetch and opaque media have no synthetic WEM children"
    );
    for (slot, (fixture, start)) in fixtures.iter().zip(starts).enumerate() {
        let slot = u64::try_from(slot)?;
        check_summary(records, fixture, start, slot, owner, origins)?;
        check_annotations(records, fixture, start, slot, owner, origins)?;
    }
    Ok(())
}

fn check_summary(
    records: &[(usize, &MediaRecord)],
    fixture: &Fixture,
    start: usize,
    slot: u64,
    owner: usize,
    origins: &Origins,
) -> TestResult {
    let (position, header) = occurrence(records, "audio-bank-wem", &[DIDX_SECTION, slot])?;
    assert_eq!(
        header.data.pointer("/payload/riff_end_exclusive"),
        Some(&json!(start + fixture.bytes.len())),
        "RIFF end is absolute within the bank Entry"
    );
    let trailing = if slot == 1 { "2" } else { "0" };
    assert_eq!(
        header.data.pointer("/payload/trailing_length"),
        Some(&json!(trailing)),
        "only DIDX-range trailing bytes"
    );
    let actual = origins.for_field(position, owner, "/data/payload/trailing_length")?;
    assert!(
        origins::contains(&actual, "dataLength"),
        "actual authored DIDX length"
    );
    assert!(
        !origins::contains(&actual, "complete-payload-length"),
        "bank total is not the WEM extent"
    );
    let actual = origins.for_field(position, owner, "/subresource/indices/0")?;
    assert!(
        !origins::contains(&actual, "tripletRouting"),
        "the section index has its own source"
    );
    let actual = origins.for_field(position, owner, "/subresource/indices/1")?;
    assert!(
        origins::contains(&actual, "tripletRouting"),
        "the next index routes the physical triplet"
    );
    let (_, chunk) = occurrence(records, "audio-bank-wem-chunk", &[DIDX_SECTION, slot, 2])?;
    assert_eq!(
        chunk.data.pointer("/payload/body_range"),
        Some(&range(start + fixture.cue.start, start + fixture.cue.end)),
        "chunk range identifies the original bytes in source.raw"
    );
    Ok(())
}

fn check_annotations(
    records: &[(usize, &MediaRecord)],
    fixture: &Fixture,
    start: usize,
    slot: u64,
    owner: usize,
    origins: &Origins,
) -> TestResult {
    let (_, point) = occurrence(
        records,
        "audio-bank-wem-cue-point",
        &[DIDX_SECTION, slot, 2, 0],
    )?;
    let cue = start + fixture.cue.start + 4;
    assert_eq!(
        point.data.pointer("/payload/point_range"),
        Some(&range(cue, cue + 24)),
        "actual bank-relative cue point span"
    );
    let (position, label) = occurrence(
        records,
        "audio-bank-wem-list-item",
        &[DIDX_SECTION, slot, 3, 0],
    )?;
    let local_start = fixture.label.start + 4;
    let local_end = local_start + 5;
    let payload = label
        .data
        .pointer("/payload/content/payload")
        .ok_or("projected label")?;
    assert_eq!(
        payload["text_range"],
        range(start + local_start, start + local_end),
        "nested text geometry"
    );
    assert_eq!(
        payload["terminator_offset"],
        start + local_end,
        "absolute NUL position"
    );
    assert_eq!(
        payload["utf8_text"], "  cue",
        "actual decoded label retains whitespace"
    );
    let actual = origins.for_field(position, owner, "/data/payload/content/payload/utf8_text")?;
    let location = json!({
        "type":"subresource","namespace":"wemAnnotationText","field":"utf8Text",
        "indices":[DIDX_SECTION.to_string(),slot.to_string(),local_start.to_string(),local_end.to_string()]
    });
    assert!(
        actual
            .iter()
            .any(|used| used["location"] == location && used["role"] == "conversion"),
        "the selected WEM-local text span carries its exact physical DIDX prefix"
    );
    let actual = origins.for_field(position, owner, "/subresource/indices/2")?;
    assert!(
        origins::contains(&actual, "chunkRouting"),
        "RIFF parent proof starts after the DIDX prefix"
    );
    Ok(())
}

fn occurrence<'rows>(
    rows: &[(usize, &'rows MediaRecord)],
    namespace: &str,
    indices: &[u64],
) -> TestResult<(usize, &'rows MediaRecord)> {
    rows.iter()
        .copied()
        .find(|(_, row)| {
            row.subresource.namespace == namespace && row.subresource.indices == indices
        })
        .ok_or_else(|| format!("missing {namespace} at {indices:?}").into())
}

fn range(start: usize, end: usize) -> Value {
    json!({"basis":{"type":"entry"},"start":start,"end_exclusive":end})
}

#[test]
fn embedded_wem_requires_unique_data_in_the_original_directory() -> TestResult {
    let folder = tempfile::tempdir()?;
    let fixture = fixture::fixture(ByteOrder::Little)?;
    let bytes = TestBank::new(8)
        .wem(MEDIA_ID, fixture.bytes.as_slice())
        .section(*b"DATA", fixture.bytes.as_slice())
        .bytes();
    let mut original = BankSource::open(Cursor::new(&bytes), u64::try_from(bytes.len())?)?;
    let sections = original.sections().collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        sections
            .iter()
            .filter(|section| section.id() == SectionId::DATA)
            .count(),
        2,
        "two actual matching DATA sections"
    );
    let archive = support::archive_with(folder.path(), [TestEntry::stored(PATH, bytes)])?;
    let output = folder.path().join("out");
    let options = ExportOptions {
        diagnostic: true,
        ..options()
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("duplicate DATA must yield diagnostic output".into());
    };
    let directory = diagnostic
        .parent()
        .ok_or("diagnostic generation directory")?;
    let rows = support::records(directory, &index, AUDIO_STREAM)?;
    let records = owned_rows(&rows, 0);
    let (_, media) = occurrence(&records, "audio-bank-media", &[DIDX_SECTION, 0])?;
    assert_eq!(
        media.data.pointer("/payload/data/status"),
        Some(&json!("ambiguous")),
        "no section was selected"
    );
    assert!(
        rows.iter()
            .all(|row| !row.subresource.namespace.starts_with("audio-bank-wem")),
        "no fabricated nested WEM"
    );
    Ok(())
}
