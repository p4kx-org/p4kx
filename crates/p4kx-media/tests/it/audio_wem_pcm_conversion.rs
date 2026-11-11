// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{Cursor, Read as _};
use std::ops::Range;
use std::path::Path;

use super::origins::Origins;
use super::{AUDIO_STREAM, TestResult, check_schema, options};
use crate::{layout_health, support};
use p4kx_extraction::bank::BankSource;
use p4kx_extraction::{
    Archive, ByteOrder, Codec, ConflictPolicy, Entry, OpenOptions, SectionId, Wem,
};
use p4kx_media::{
    AudioOutput, ExportOptions, ExportOutcome, Index, MediaError, MediaKind, MediaRecord,
};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestCodec, TestEntry, TestWem, test_key};

#[path = "audio_wem_pcm_conversion_assertions.rs"]
mod assertions;

use self::assertions::{check_manifest, check_mapping, check_refusal, check_wav};

const LOOSE: &str = "selected.wem";
const BANK: &str = "selected.bnk";
const LOOSE_ROW: &str = "audio-wem-conversion";
const BANK_ROW: &str = "audio-bank-wem-conversion";
const SAMPLES: [i16; 4] = [i16::MIN, -37, 256, i16::MAX];
const MEDIA_ID: u32 = 49;
const BANK_INDICES: [u64; 2] = [1, 1];
const RATE: u32 = 48_000;

#[test]
fn requested_pcm_wav_streams_loose_and_banked_sources_with_exact_artifact_origins() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = archive(folder.path())?;
    let original = original_geometry(&archive)?;
    let raw = folder.path().join("raw");
    let raw_index = support::published(p4kx_media::export(&archive, &raw, &options())?)?;
    let rows = support::records(&raw, &raw_index, AUDIO_STREAM)?;
    assert!(
        rows.iter().all(|row| !is_conversion(row)),
        "default has no conversion mappings"
    );
    assert_eq!(
        converted_tally(&raw_index)?,
        (0, 0, 0),
        "default has no WAV outputs"
    );

    let output = folder.path().join("wav");
    let index = support::published(p4kx_media::export(&archive, &output, &wav_options())?)?;
    let rows = support::records(&output, &index, AUDIO_STREAM)?;
    check_schema(&rows)?;
    let directory = output.join(&index.artifact_base);
    let origins = Origins::read(&directory)?;
    let converted: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| is_conversion(row))
        .collect();
    let [loose, bank] = converted.as_slice() else {
        return Err("only the two complete original WEMs have conversion mappings".into());
    };
    let [original_loose, original_bank] = &original;
    let loose_artifact = check_mapping(*loose, LOOSE_ROW, original_loose, &origins, 0)?;
    let bank_artifact = check_mapping(*bank, BANK_ROW, original_bank, &origins, 1)?;
    assert_eq!(
        loose_artifact.path, bank_artifact.path,
        "decoded content deduplicates"
    );
    assert_eq!(
        converted_tally(&index)?,
        (2, 1, 1),
        "manifest retains both conversions"
    );
    check_wav(&directory, &loose_artifact)?;
    check_manifest(&directory, &index, &loose_artifact, &original, &origins)
}

#[test]
fn requested_wav_refuses_unsupported_codec_and_preserves_the_raw_generation() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bytes = TestWem::encoded(TestCodec::Vorbis, 1, RATE, 8).bytes();
    let archive =
        support::archive_with(folder.path(), [TestEntry::stored(LOOSE, bytes.as_slice())])?;
    let [entry] = archive.entries() else {
        return Err("one physical WEM".into());
    };
    let original = original_bytes(&archive, entry)?;
    assert_eq!(
        original, bytes,
        "the original compressed payload is fully readable"
    );
    assert_eq!(
        Wem::parse(&original)?.codec(),
        Codec::Vorbis,
        "known compressed source"
    );
    let output = folder.path().join("out");
    let raw = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let raw_index = fs::read(output.join("media-index.json"))?;
    let overwrite = ExportOptions {
        conflict: ConflictPolicy::Overwrite,
        ..wav_options()
    };
    let failure = p4kx_media::export(&archive, &output, &overwrite)
        .err()
        .ok_or("unsupported conversion cannot publish normally")?;
    assert!(
        matches!(
            layout_health::leaf(&failure),
            MediaError::ProjectionLoss {
                kind: MediaKind::Audio,
                ..
            }
        ),
        "unsupported output causes typed Audio projection loss"
    );
    check_diagnostic(&archive, &output, overwrite, &original)?;
    assert_eq!(
        fs::read(output.join("media-index.json"))?,
        raw_index,
        "raw index remains current"
    );
    assert!(
        output.join(raw.artifact_base).exists(),
        "original complete raw output is retained"
    );
    Ok(())
}

fn check_diagnostic(
    archive: &Archive,
    output: &Path,
    overwrite: ExportOptions,
    original: &[u8],
) -> TestResult {
    let diagnostic = ExportOptions {
        diagnostic: true,
        ..overwrite
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(archive, output, &diagnostic)?
    else {
        return Err("unsupported output diagnostic generation".into());
    };
    let directory = diagnostic
        .parent()
        .ok_or("diagnostic generation directory")?;
    let rows = support::records(directory, &index, AUDIO_STREAM)?;
    check_schema(&rows)?;
    check_refusal(&rows, directory, original)?;
    assert_eq!(
        converted_tally(&index)?,
        (0, 0, 0),
        "no unsuccessful artifact is placed"
    );
    Ok(())
}

struct Original {
    base: u64,
    samples: Range<u64>,
}

fn archive(folder: &Path) -> TestResult<Archive> {
    let source = TestWem::pcm(TestCodec::Pcm, 1, RATE, &SAMPLES)
        .smpl_loop(0, 1, 2)
        .data([pcm_bytes(), vec![0xff]].concat());
    let plain = source.bytes();
    let banked = source.big_endian().bytes();
    let bank = TestBank::new(7)
        .wem(MEDIA_ID, b"PLUGopaque".as_slice())
        .wem(MEDIA_ID, banked)
        .wem(MEDIA_ID, b"RIFF\x50\0\0\0WAVE".as_slice())
        .bytes();
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(LOOSE, plain))
        .entry(TestEntry::zstd(BANK, bank).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn original_bytes(archive: &Archive, entry: &Entry) -> TestResult<Vec<u8>> {
    let mut result = Vec::new();
    archive
        .open_entry_verified(entry)?
        .reject_overlong()
        .read_to_end(&mut result)?;
    Ok(result)
}

fn original_geometry(archive: &Archive) -> TestResult<[Original; 2]> {
    let [loose, bank] = archive.entries() else {
        return Err("two physical archive sources".into());
    };
    assert!(
        !loose.is_encrypted() && bank.is_encrypted(),
        "original payload markers differ"
    );
    let plain = original_bytes(archive, loose)?;
    let bytes = original_bytes(archive, bank)?;
    let mut bank = BankSource::open(Cursor::new(&bytes), u64::try_from(bytes.len())?)?;
    let sections = bank.sections().collect::<Result<Vec<_>, _>>()?;
    let [_, didx, data] = sections.as_slice() else {
        return Err("one actual DIDX and DATA".into());
    };
    assert_eq!(
        (didx.id(), data.id()),
        (SectionId::DIDX, SectionId::DATA),
        "section identities"
    );
    let mut cursor = bank.didx(didx)?;
    let media = cursor.nth(1).ok_or("second physical triplet")??;
    let selected = cursor.inspect_media(&media, data)?;
    assert_eq!(
        [didx.ordinal(), media.ordinal()],
        BANK_INDICES,
        "original routing coordinates"
    );
    assert_eq!(
        media.media_id(),
        MEDIA_ID,
        "selection uses a physical occurrence of a repeated ID"
    );
    let base = selected.range.start;
    let original = bytes
        .get(usize::try_from(base)?..usize::try_from(selected.range.end)?)
        .ok_or("original DIDX range")?;
    Ok([
        original_frames(&plain, 0, ByteOrder::Little)?,
        original_frames(original, base, ByteOrder::Big)?,
    ])
}

fn original_frames(bytes: &[u8], base: u64, order: ByteOrder) -> TestResult<Original> {
    let wem = Wem::parse(bytes)?;
    let data = wem.metadata().data_chunk();
    assert_eq!(
        (wem.codec(), wem.byte_order()),
        (Codec::Pcm, order),
        "actual PCM declaration"
    );
    assert_eq!(wem.sample_count(), Some(4), "four original complete frames");
    let expected: Vec<_> = SAMPLES
        .into_iter()
        .flat_map(|sample| match order {
            ByteOrder::Little => sample.to_le_bytes(),
            ByteOrder::Big => sample.to_be_bytes(),
        })
        .collect();
    assert_eq!(
        wem.data().strip_suffix(&[0xff]),
        Some(expected.as_slice()),
        "authored samples and partial byte"
    );
    Ok(Original {
        base,
        samples: data.body_offset..data.body_offset + u64::try_from(expected.len())?,
    })
}

fn is_conversion(row: &MediaRecord) -> bool {
    matches!(row.subresource.namespace.as_str(), LOOSE_ROW | BANK_ROW)
}

const fn indices(owner: usize) -> &'static [u64] {
    if owner == 0 { &[] } else { &BANK_INDICES }
}

fn wav_options() -> ExportOptions {
    ExportOptions {
        audio_output: AudioOutput::Wav,
        ..options()
    }
}

fn converted_tally(index: &Index) -> TestResult<(u64, u64, u64)> {
    let counts = index
        .kinds
        .first()
        .ok_or("selected Audio kind")?
        .counts
        .converted_outputs;
    Ok((counts.enumerated, counts.written, counts.skipped))
}

fn pcm_bytes() -> Vec<u8> {
    SAMPLES.into_iter().flat_map(i16::to_le_bytes).collect()
}
