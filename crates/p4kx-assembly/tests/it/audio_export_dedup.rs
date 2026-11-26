// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A WEM reached through several variants is exported once, from its full copy.

use std::error::Error;
use std::fs;

use p4kx_assembly::audio::{
    ArchiveAudio, ExportOptions, ExportedSound, SoundFormat, TriggerSounds,
};
use p4kx_audio::Wem;
use p4kx_extraction::{Archive, Checksum, ConflictPolicy};
use p4kx_testkit::{
    ArchiveBuilder, TestAtl, TestBank, TestCodec, TestEntry, TestHircObject, TestWem, test_signal,
    wwise_short_id,
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

type TestResult = Result<(), Box<dyn Error>>;

const PLAY: u16 = 0x0403;
const OPTIONS: ExportOptions = ExportOptions {
    format: SoundFormat::Wav,
    policy: ConflictPolicy::Overwrite,
};

fn pcm() -> Vec<u8> {
    TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(480, 1)).bytes()
}

fn bank(name: &str, event: &str, wem: Vec<u8>, storage: u8) -> Vec<u8> {
    let size = u32::try_from(wem.len()).unwrap_or(0);
    TestBank::new(wwise_short_id(name))
        .wem(7001, wem)
        .object(TestHircObject::event(wwise_short_id(event), &[1]))
        .object(TestHircObject::action(1, PLAY, 10))
        .object(TestHircObject::sound(10, 7001, storage, size))
        .bytes()
}

fn export_mix(first: (&str, Vec<u8>, u8), second: (&str, Vec<u8>, u8)) -> TestResult {
    let full_in_second = second.2 == 0;
    let full = if full_in_second { &second.1 } else { &first.1 };
    let mut expected = Vec::new();
    Wem::parse(full.as_slice())?.write_wav(&mut expected)?;
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC.xml",
            TestAtl::new("SC")
                .trigger("Play_Mix", &["Play_First", "Play_Second"])
                .to_cryxml(),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\First.bnk",
            bank("First", "Play_First", first.1, first.2),
        ))
        .entry(
            TestEntry::zstd(
                "Data\\Sounds\\wwise\\Second.bnk",
                bank("Second", "Play_Second", second.1, second.2),
            )
            .encrypted(),
        )
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let audio = ArchiveAudio::new(&archive);
    let sounds = audio.resolve_trigger("Play_Mix").ok_or("resolves")?;
    assert_eq!(sounds.variants().len(), 2, "two variants lead to the WEM");
    let out = tempfile::tempdir()?;
    let triggers: Vec<&TriggerSounds<'_>> = vec![&sounds];

    let report = audio.export_sounds(&triggers, out.path(), OPTIONS)?;
    let [sound] = report.sounds.as_slice() else {
        return Err("one file for the WEM".into());
    };
    assert_eq!(sound.format, SoundFormat::Wav, "the full copy converts");
    assert_eq!(sound.unconverted, None, "nothing is left unconverted");
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_bound_pcm(sound, &expected, full_in_second)
}

fn assert_bound_pcm(sound: &ExportedSound, expected: &[u8], encrypted: bool) -> TestResult {
    assert!(
        sound.origin_receipt_failure.is_none(),
        "companion publication succeeded"
    );
    let companion = sound
        .origin_receipt
        .as_deref()
        .ok_or("published companion")?;
    let installed = fs::read(&sound.path)?;
    assert_eq!(installed, expected, "the standard converter's PCM bytes");
    let content = fs::read_to_string(companion)?;
    let rows: Vec<Value> = content
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let binding = rows.first().ok_or("artifact byte binding")?;
    let sha = Checksum::Sha256(Sha256::digest(&installed).into()).hex();
    assert_eq!(
        binding.pointer("/binding/sha256"),
        Some(&json!(sha)),
        "the companion binds the installed bytes"
    );
    let samples = rows
        .iter()
        .find(|row| row.pointer("/address/field") == Some(&json!("pcmSamples")))
        .ok_or("the WAV sample field")?;
    assert_eq!(
        samples.get("sourceSetComplete"),
        Some(&json!(true)),
        "the sample input is known"
    );
    assert_eq!(
        samples.get("containsEncryptedSource"),
        Some(&json!(encrypted)),
        "the chosen complete member supplies the bytes"
    );
    let references = samples
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("sample uses")?;
    assert!(
        rows.iter()
            .filter_map(|row| row.get("sourceUse"))
            .any(|used| {
                used.get("id").is_some_and(|id| references.contains(id))
                    && used.pointer("/location/namespace").and_then(Value::as_str)
                        == Some("bnkDidx")
                    && used.pointer("/location/field").and_then(Value::as_str) == Some("dataOffset")
                    && used.get("role").and_then(Value::as_str) == Some("selector")
            }),
        "sample dependencies contain the original bank member selection"
    );
    Ok(())
}

#[test]
fn a_full_copy_wins_over_an_earlier_prefetch_only_copy() -> TestResult {
    let full = pcm();
    let start = full.get(..40).ok_or("long enough")?.to_vec();
    export_mix(("First", start, 1), ("Second", full, 0))
}

#[test]
fn a_full_copy_stays_when_a_prefetch_only_copy_follows() -> TestResult {
    let full = pcm();
    let start = full.get(..40).ok_or("long enough")?.to_vec();
    export_mix(("First", full, 0), ("Second", start, 1))
}
