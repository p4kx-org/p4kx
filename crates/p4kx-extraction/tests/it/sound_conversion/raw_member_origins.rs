// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_audio::{EmbeddedWem, Soundbank};
use p4kx_extraction::{
    Archive, AudioConversion, Checksum, ConflictPolicy, Exclusion, ExtractOptions, FolderSource,
    RawEntryArtifact, convert_folder, extract,
};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::bank_origins::{expect_origin, expect_selection};
use super::fixture::{BANK, Fixture, TestResult, partial_files, pcm, prefetch};
use crate::bulk_wav_origin::read_companion;

const BANK_PATH: &str = "Data/Sounds/wwise/Ship.bnk";
const MIDI: &[u8] = b"MThd\0\0\0\x06\0\x01\0\x01\x01\xE0";
const MIDI_PATH: &str = "Data/Sounds/wwise/Ship/10.mid";
const PREFETCH_PATH: &str = "Data/Sounds/wwise/Ship/20.wem";
const COMPLETE_PATH: &str = "Data/Sounds/wwise/Ship/30.wem";

#[test]
fn unchanged_archive_bank_members_bind_the_encrypted_bank_and_their_exact_bytes() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let target = folder.path().join("out");
    fs::create_dir(&target)?;
    let bytes = mixed_bank();
    ArchiveBuilder::v2()
        .entry(TestEntry::zstd(BANK, bytes.as_slice()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entries: Vec<_> = archive.entries().iter().collect();
    let report = extract(&archive, &entries, &target, &options(AudioConversion::Raw));
    assert!(report.is_complete(), "{report:?}");
    assert_eq!(report.converted(), Some(0), "raw members stay raw");
    assert!(report.sound_artifacts().is_empty(), "nothing converted");
    let [bank_file, midi, prefetched, complete] = report.raw_artifacts() else {
        return Err("the bank and three unchanged member receipts".into());
    };
    assert_eq!(bank_file.path, BANK_PATH, "the parent bank");
    let bank = Soundbank::parse(bytes.as_slice())?;
    let encrypted = json!(true);
    check_member(&target, midi, &bank, 10, MIDI_PATH, &encrypted)?;
    check_member(&target, prefetched, &bank, 20, PREFETCH_PATH, &encrypted)?;
    check_member(&target, complete, &bank, 30, COMPLETE_PATH, &encrypted)
}

#[test]
fn folder_bank_preserves_midi_and_prefetch_under_ogg_conversion() -> TestResult {
    let folder = tempfile::tempdir()?;
    let input = folder.path().join("in");
    let target = folder.path().join("out");
    let path = input.join(BANK_PATH);
    fs::create_dir_all(path.parent().ok_or("input bank folder")?)?;
    let bytes = mixed_bank();
    fs::write(path, &bytes)?;
    let source = FolderSource::scan(&input, None)?;
    let selected = source.select(None, &Exclusion::default());
    let report = convert_folder(
        &source,
        &selected,
        false,
        &target,
        &options(AudioConversion::Ogg),
    )?;
    assert!(report.is_complete(), "{report:?}");
    assert_eq!(report.converted(), Some(1), "the complete PCM converts");
    let [bank_file, midi, prefetched] = report.raw_artifacts() else {
        return Err("the bank, MIDI and prefetch receipts".into());
    };
    assert_eq!(bank_file.path, BANK_PATH, "the parent bank");
    let bank = Soundbank::parse(bytes.as_slice())?;
    check_member(&target, midi, &bank, 10, MIDI_PATH, &Value::Null)?;
    check_member(&target, prefetched, &bank, 20, PREFETCH_PATH, &Value::Null)
}

#[test]
fn a_foreign_raw_member_companion_blocks_replacement() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::stored(BANK, mixed_bank())])?;
    let midi = fixture.target.join(MIDI_PATH);
    fs::create_dir_all(midi.parent().ok_or("member folder")?)?;
    let companion = fixture
        .target
        .join(format!("{MIDI_PATH}.p4kx-origin.jsonl"));
    fs::write(midi, b"incumbent")?;
    fs::write(&companion, b"foreign")?;

    let report = fixture.run(AudioConversion::Raw, ConflictPolicy::Overwrite);
    assert_eq!(report.failures().len(), 1, "{report:?}");
    let [bank] = report.raw_artifacts() else {
        return Err("only the parent bank committed".into());
    };
    assert_eq!(bank.path, BANK_PATH);
    assert_eq!(fixture.read(MIDI_PATH)?, b"incumbent", "incumbent bytes");
    assert_eq!(fs::read(companion)?, b"foreign", "foreign companion");
    assert!(partial_files(&fixture.target)?.is_empty(), "no staged file");
    Ok(())
}

#[test]
fn raw_member_replay_keeps_valid_proof_and_replaces_stale_output() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::stored(BANK, mixed_bank())])?;
    let report = fixture.run(AudioConversion::Raw, ConflictPolicy::IfChanged);
    assert!(report.is_complete(), "{report:?}");
    let [_, midi, _, _] = report.raw_artifacts() else {
        return Err("the bank and three raw member receipts".into());
    };
    let companion = midi.origin_receipt.as_deref().ok_or("MIDI companion")?;
    let initial = fixture.read(companion)?;
    let replay = fixture.run(AudioConversion::Raw, ConflictPolicy::IfChanged);
    assert_eq!(replay.skipped(), &[BANK], "{replay:?}");
    assert!(
        replay.raw_artifacts().is_empty(),
        "kept members are not re-published"
    );
    assert_eq!(fixture.read(companion)?, initial, "the current proof stays");

    fs::write(fixture.target.join(MIDI_PATH), b"changed")?;
    let replaced = fixture.run(AudioConversion::Raw, ConflictPolicy::IfChanged);
    assert!(replaced.is_complete(), "{replaced:?}");
    let [midi] = replaced.raw_artifacts() else {
        return Err("one repaired member receipt".into());
    };
    let bytes = mixed_bank();
    let bank = Soundbank::parse(bytes.as_slice())?;
    check_member(&fixture.target, midi, &bank, 10, MIDI_PATH, &json!(false))?;
    assert!(partial_files(&fixture.target)?.is_empty(), "no staged file");
    Ok(())
}

fn mixed_bank() -> Vec<u8> {
    TestBank::new(11)
        .wem(30, pcm())
        .wem(20, prefetch())
        .wem(10, MIDI)
        .bytes()
}

fn options(audio: AudioConversion) -> ExtractOptions<'static> {
    ExtractOptions {
        convert: true,
        policy: ConflictPolicy::IfChanged,
        audio,
        ..ExtractOptions::default()
    }
}

fn check_member(
    target: &Path,
    artifact: &RawEntryArtifact,
    bank: &Soundbank<&[u8]>,
    id: u32,
    path: &str,
    encryption: &Value,
) -> TestResult {
    assert_eq!(artifact.path, path, "unchanged member file name");
    let member = bank.wem(id).ok_or("bank member")?;
    let output = fs::read(target.join(path))?;
    assert_eq!(output, member.bytes(), "retained member bytes");
    let sha = Checksum::Sha256(Sha256::digest(&output).into()).hex();
    assert_eq!(artifact.digest.sha256(), sha, "reported SHA-256");
    assert_eq!(
        artifact.digest.byte_length(),
        output.len() as u64,
        "reported length of the retained member"
    );
    let companion = artifact
        .origin_receipt
        .as_deref()
        .ok_or("member companion")?;
    assert_eq!(
        companion,
        format!("{path}.p4kx-origin.jsonl"),
        "receipt beside the retained member"
    );
    let rows = read_companion(&fs::read(target.join(companion))?)?;
    let binding = rows.first().ok_or("receipt header")?;
    assert_eq!(
        binding.pointer("/binding/sha256"),
        Some(&json!(sha)),
        "receipt binds the retained member bytes"
    );
    assert_eq!(
        binding.pointer("/binding/byteLength"),
        Some(&json!(output.len().to_string())),
        "receipt binds the full length of the retained member",
    );
    check_payload(&rows, member, encryption)
}

fn check_payload(rows: &[Value], member: EmbeddedWem<'_>, encryption: &Value) -> TestResult {
    let payload = rows
        .iter()
        .find(|row| row.pointer("/address/type").and_then(Value::as_str) == Some("opaquePayload"))
        .ok_or("one opaque payload origin")?;
    let uses = expect_selection(rows, payload, member, encryption)?;
    assert_eq!(uses.len(), 5, "four selectors and the payload");
    let range = member.source().byte_range();
    expect_origin(
        &uses,
        "bnkEmbeddedWem",
        "payload",
        &[range.start, range.end],
        "payload",
    );
    Ok(())
}
