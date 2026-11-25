// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::ops::Range;
use std::path::Path;

use p4kx_audio::{EmbeddedWem, Soundbank};
use p4kx_extraction::{
    Archive, AudioConversion, Checksum, ConflictPolicy, ConvertedSoundArtifact, Exclusion,
    ExtractOptions, ExtractionReport, FolderSource, convert_folder, extract,
};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::fixture::{BANK, Fixture, TestResult, partial_files, pcm, vorbis};
use crate::bulk_wav_origin::{field, field_uses, read_companion};

const BANK_FILE: &str = "Data/Sounds/wwise/Ship.bnk";
const OGG: &str = "Data/Sounds/wwise/Ship/10.ogg";
const WAV: &str = "Data/Sounds/wwise/Ship/20.wav";

#[test]
fn archive_bank_outputs_bind_each_selected_didx_triplet_and_wem_extent() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("Data.p4k");
    let out = dir.path().join("out");
    fs::create_dir(&out)?;
    let bytes = bank();
    ArchiveBuilder::v2()
        .entry(TestEntry::zstd(BANK, bytes.as_slice()).encrypted())
        .write_to(&input)?;
    let archive = Archive::open_with_key(input, Some(p4kx_testkit::test_key()))?;
    let entries: Vec<_> = archive.entries().iter().collect();
    let report = extract(
        &archive,
        &entries,
        &out,
        &options(ConflictPolicy::IfChanged),
    );
    expect_member_receipts(&report, &bytes, &out, &json!(true))?;
    let held = extract(
        &archive,
        &entries,
        &out,
        &options(ConflictPolicy::IfChanged),
    );
    assert_eq!(
        held.skipped(),
        &[BANK],
        "the bank and both members were retained"
    );
    assert!(held.sound_artifacts().is_empty(), "no re-publication");
    Ok(())
}

#[test]
fn folder_bank_receipts_distinguish_observed_files_from_encrypted_archive_entries() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("in");
    let out = dir.path().join("out");
    let bytes = bank();
    let path = input.join(BANK_FILE);
    fs::create_dir_all(path.parent().ok_or("bank parent")?)?;
    fs::write(path, &bytes)?;
    let source = FolderSource::scan(&input, None)?;
    let selected = source.select(None, &Exclusion::default());
    let report = convert_folder(
        &source,
        &selected,
        false,
        &out,
        &options(ConflictPolicy::Skip),
    )?;
    expect_member_receipts(&report, &bytes, &out, &Value::Null)
}

#[test]
fn a_foreign_member_companion_leaves_the_incumbent_and_prior_receipt_intact() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::stored(BANK, bank())])?;
    let protected = fixture.target.join(WAV);
    fs::create_dir_all(protected.parent().ok_or("bank output folder")?)?;
    let foreign = fixture.target.join(format!("{WAV}.p4kx-origin.jsonl"));
    fs::write(protected, b"incumbent")?;
    fs::write(&foreign, b"foreign")?;
    let report = fixture.run(AudioConversion::Ogg, ConflictPolicy::Overwrite);
    assert_eq!(report.failures().len(), 1, "{report:?}");
    let [first] = report.sound_artifacts() else {
        return Err("only the completed Ogg receipt".into());
    };
    assert_eq!(first.path, OGG, "first decoded member");
    assert!(
        fixture.exists(&first.origin_receipt),
        "first sidecar stays installed"
    );
    assert_eq!(
        fixture.read(WAV)?,
        b"incumbent",
        "failed second member was not replaced"
    );
    assert_eq!(
        fs::read(foreign)?,
        b"foreign",
        "foreign companion was not replaced"
    );
    assert!(
        partial_files(&fixture.target)?.is_empty(),
        "no staged output remained"
    );
    Ok(())
}

fn bank() -> Vec<u8> {
    TestBank::new(11).wem(20, pcm()).wem(10, vorbis()).bytes()
}

fn options(policy: ConflictPolicy) -> ExtractOptions<'static> {
    ExtractOptions {
        convert: true,
        policy,
        audio: AudioConversion::Ogg,
        ..ExtractOptions::default()
    }
}

fn expect_member_receipts(
    report: &ExtractionReport,
    bytes: &[u8],
    out: &Path,
    encryption: &Value,
) -> TestResult {
    assert!(report.is_complete(), "{report:?}");
    assert_eq!(
        fs::read(out.join(BANK_FILE))?,
        bytes,
        "the original bank bytes"
    );
    let [ogg_artifact, wav_artifact] = report.sound_artifacts() else {
        return Err("one receipt per converted bank member".into());
    };
    assert_eq!(ogg_artifact.path, OGG, "Ogg artifact in path order");
    assert_eq!(wav_artifact.path, WAV, "WAV artifact in path order");
    let bank = Soundbank::parse(bytes)?;
    let ogg = bank.wem(10).ok_or("Vorbis DIDX member")?;
    let wav = bank.wem(20).ok_or("PCM DIDX member")?;
    assert!(
        ogg.source().triplet_offset() > wav.source().triplet_offset(),
        "DIDX differs from ID order"
    );
    expect_ogg(out, ogg, ogg_artifact, encryption)?;
    expect_wav(out, wav, wav_artifact, encryption)
}

fn expect_ogg(
    out: &Path,
    member: EmbeddedWem<'_>,
    artifact: &ConvertedSoundArtifact,
    encryption: &Value,
) -> TestResult {
    let wem = member.parse()?;
    let mut expected = Vec::new();
    wem.write_ogg(&mut expected)?;
    let rows = expect_bytes(out, artifact, &expected)?;
    let group = field(&rows, "audio")?;
    let uses = expect_selection(&rows, group, member, encryption)?;
    let data = wem.metadata().data_chunk();
    let setup = u16::from_le_bytes(*wem.data().first_chunk::<2>().ok_or("setup size")?);
    let start = data.body_offset + 2 + u64::from(setup);
    expect_nested_range(
        &uses,
        member,
        "bnkWem.wemVorbis",
        "audioReplayed",
        start..data.body_end_exclusive(),
    );
    Ok(())
}

fn expect_wav(
    out: &Path,
    member: EmbeddedWem<'_>,
    artifact: &ConvertedSoundArtifact,
    encryption: &Value,
) -> TestResult {
    let wem = member.parse()?;
    let mut expected = Vec::new();
    wem.write_wav(&mut expected)?;
    let rows = expect_bytes(out, artifact, &expected)?;
    let group = field(&rows, "pcmSamples")?;
    let uses = expect_selection(&rows, group, member, encryption)?;
    let data = wem.metadata().data_chunk();
    expect_nested_range(
        &uses,
        member,
        "bnkWem.wemPcmFrames",
        "decodedAudio",
        data.body_offset..data.body_end_exclusive(),
    );
    Ok(())
}

fn expect_bytes(
    out: &Path,
    artifact: &ConvertedSoundArtifact,
    expected: &[u8],
) -> Result<Vec<Value>, Box<dyn Error>> {
    let written = fs::read(out.join(&artifact.path))?;
    assert_eq!(written, expected, "same bytes as the normal codec writer");
    let sha = Checksum::Sha256(Sha256::digest(written).into()).hex();
    assert_eq!(artifact.digest.sha256(), sha, "reported artifact digest");
    let rows = read_companion(&fs::read(out.join(&artifact.origin_receipt))?)?;
    let header = rows.first().ok_or("origin header")?;
    assert_eq!(
        header.pointer("/binding/sha256"),
        Some(&json!(sha)),
        "receipt binding"
    );
    Ok(rows)
}

pub(super) fn expect_selection<'a>(
    rows: &'a [Value],
    group: &Value,
    member: EmbeddedWem<'_>,
    encryption: &Value,
) -> Result<Vec<&'a Value>, Box<dyn Error>> {
    assert_eq!(
        group.get("containsEncryptedSource"),
        Some(encryption),
        "bank source encryption"
    );
    assert_eq!(
        group.get("sourceSetComplete"),
        Some(&json!(true)),
        "complete source selection"
    );
    let uses = field_uses(rows, group)?;
    let triplet = member.source().triplet_offset();
    for (offset, field) in [(0, "mediaId"), (4, "dataOffset"), (8, "dataLength")] {
        expect_origin(
            &uses,
            "bnkDidx",
            field,
            &[triplet + offset, triplet + offset + 4],
            "selector",
        );
    }
    let data = member.source().data_body_offset();
    expect_origin(
        &uses,
        "bnkData",
        "bodyLength",
        &[data - 4, data],
        "selector",
    );
    let bytes = member.source().byte_range();
    assert!(
        uses.iter()
            .filter(|used| {
                used.pointer("/location/namespace")
                    .and_then(Value::as_str)
                    .is_some_and(|namespace| namespace.starts_with("bnkWem."))
            })
            .all(|used| {
                used.pointer("/location/indices/0") == Some(&json!(bytes.start.to_string()))
                    && used.pointer("/location/indices/1") == Some(&json!(bytes.end.to_string()))
            }),
        "nested sites refer to the selected member's extent"
    );
    Ok(uses)
}

fn expect_nested_range(
    uses: &[&Value],
    member: EmbeddedWem<'_>,
    namespace: &str,
    field: &str,
    range: Range<u64>,
) {
    let extent = member.source().byte_range();
    expect_origin(
        uses,
        namespace,
        field,
        &[extent.start, extent.end, range.start, range.end],
        "conversion",
    );
}

pub(super) fn expect_origin(
    uses: &[&Value],
    namespace: &str,
    field: &str,
    coordinates: &[u64],
    role: &str,
) {
    let indices = json!(coordinates.iter().map(u64::to_string).collect::<Vec<_>>());
    assert!(
        uses.iter().any(|used| {
            used.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && used.pointer("/location/indices") == Some(&indices)
                && used.get("role").and_then(Value::as_str) == Some(role)
        }),
        "selected {namespace}/{field} at {coordinates:?}: {uses:?}"
    );
}
