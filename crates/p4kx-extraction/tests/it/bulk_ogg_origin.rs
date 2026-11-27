// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{
    AudioConversion, ConflictPolicy, ConversionError, ConvertedSoundArtifact, Exclusion,
    ExtractOptions, ExtractionError, FolderSource, convert_folder, extract,
};

use super::{
    Archive, ArchiveBuilder, AudioError, Checksum, Digest, Sha256, TestEntry, TestResult,
    TestVorbis, Value, Wem, assert_output_addresses, assert_switch_timing_words,
    assert_vorbis_packet_sources, json, switch_fixture,
};

const ENTRY: &str = "Data\\Sounds\\origin.wem";
const ARCHIVE_OUTPUT: &str = "Data/Sounds/origin.ogg";

#[test]
fn extraction_keeps_original_ogg_bytes_and_binds_encrypted_vorbis_sources() -> TestResult {
    let dir = tempfile::tempdir()?;
    let fixture = TestVorbis::new(2, 24_000, &[false; 40]);
    let bytes = fixture.wem().bytes();
    let input = dir.path().join("Data.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::zstd(ENTRY, bytes.as_slice()).encrypted())
        .write_to(&input)?;
    let archive = Archive::open_with_key(input, Some(p4kx_testkit::test_key()))?;
    let selection: Vec<_> = archive.entries().iter().collect();
    let target = dir.path().join("out");
    fs::create_dir(&target)?;
    let options = options();
    let report = extract(&archive, &selection, &target, &options);
    assert!(report.is_complete(), "{report:?}");
    let [artifact] = report.sound_artifacts() else {
        return Err("one archive Ogg receipt".into());
    };
    let wem = Wem::parse(&bytes)?;
    let rows = assert_written(artifact, &target, ARCHIVE_OUTPUT, &wem, &json!(true))?;
    assert_vorbis_packet_sources(&rows, &wem, &fixture)?;
    let companion = target.join(&artifact.origin_receipt);
    let original = fs::read(&companion)?;
    let again = extract(&archive, &selection, &target, &options);
    assert_eq!(again.skipped(), &[ENTRY], "unchanged artifact retained");
    assert!(
        again.sound_artifacts().is_empty(),
        "no receipt was published"
    );
    assert_eq!(fs::read(companion)?, original, "original receipt retained");
    Ok(())
}

#[test]
fn folder_conversion_binds_exact_switch_timing_words_with_unknown_encryption() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("in");
    let target = dir.path().join("out");
    fs::create_dir(&input)?;
    let bytes = switch_fixture()?;
    fs::write(input.join("switch.wem"), &bytes)?;
    let source = FolderSource::scan(&input, None)?;
    let selection = source.select(None, &Exclusion::default());
    let report = convert_folder(&source, &selection, false, &target, &options())?;
    assert!(report.is_complete(), "{report:?}");
    let [artifact] = report.sound_artifacts() else {
        return Err("one folder Ogg receipt".into());
    };
    let wem = Wem::parse(&bytes)?;
    let rows = assert_written(artifact, &target, "switch.ogg", &wem, &Value::Null)?;
    assert_switch_timing_words(&rows, &wem)
}

#[test]
fn malformed_ogg_keeps_the_raw_wem_and_typed_conversion_failure() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("in");
    let target = dir.path().join("out");
    fs::create_dir(&input)?;
    let bytes = TestVorbis::new(2, 24_000, &[false; 40])
        .wem()
        .data(vec![0xff])
        .bytes();
    fs::write(input.join("broken.wem"), &bytes)?;
    let source = FolderSource::scan(&input, None)?;
    let selection = source.select(None, &Exclusion::default());
    let report = convert_folder(&source, &selection, false, &target, &options())?;
    let [failure] = report.unconverted() else {
        return Err("one failed Vorbis conversion".into());
    };
    let ExtractionError::Sound(error) = failure.error() else {
        return Err("sound failure".into());
    };
    assert!(
        matches!(
            error.as_ref(),
            ConversionError::Audio {
                source: AudioError::InvalidVorbis(_),
                ..
            }
        ),
        "typed Vorbis fallback: {error:?}"
    );
    assert!(report.failures().is_empty(), "{:?}", report.failures());
    assert!(
        report.sound_artifacts().is_empty(),
        "no written codec evidence"
    );
    assert_eq!(fs::read(target.join("broken.wem"))?, bytes, "raw fallback");
    let [artifact] = report.raw_artifacts() else {
        return Err("one unchanged fallback artifact".into());
    };
    let receipt = "broken.wem.p4kx-origin.jsonl";
    assert_eq!(artifact.path, "broken.wem");
    assert_eq!(artifact.origin_receipt.as_deref(), Some(receipt));
    assert!(
        target.join(receipt).is_file(),
        "the kept bytes have a receipt"
    );
    assert_eq!(
        fs::read_dir(target)?.count(),
        2,
        "the raw WEM and its proof are the only outputs"
    );
    Ok(())
}

fn options() -> ExtractOptions<'static> {
    ExtractOptions {
        convert: true,
        audio: AudioConversion::Ogg,
        policy: ConflictPolicy::IfChanged,
        ..ExtractOptions::default()
    }
}

fn assert_written(
    artifact: &ConvertedSoundArtifact,
    target: &Path,
    expected_path: &str,
    wem: &Wem<impl AsRef<[u8]>>,
    encryption: &Value,
) -> Result<Vec<Value>, Box<dyn Error>> {
    assert_eq!(
        artifact.path, expected_path,
        "target-relative artifact path"
    );
    assert_eq!(
        artifact.origin_receipt,
        format!("{expected_path}.p4kx-origin.jsonl"),
        "target-relative receipt path"
    );
    let bytes = fs::read(target.join(&artifact.path))?;
    let mut expected = Vec::new();
    let report = wem.write_ogg(&mut expected)?;
    assert_eq!(bytes, expected, "unchanged Ogg output from the same WEM");
    assert_eq!(
        artifact.digest.byte_length(),
        report.size,
        "reported output size"
    );
    let sha = Checksum::Sha256(Sha256::digest(&bytes).into()).hex();
    assert_eq!(artifact.digest.sha256(), sha, "reported output digest");
    let rows: Vec<Value> = fs::read_to_string(target.join(&artifact.origin_receipt))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let header = rows.first().ok_or("receipt binding")?;
    assert_eq!(
        header.pointer("/binding/sha256"),
        Some(&json!(sha)),
        "receipt digest"
    );
    assert_output_addresses(&rows, &bytes)?;
    for row in rows.iter().filter(|item| item.get("address").is_some()) {
        assert_eq!(
            row.get("containsEncryptedSource"),
            Some(encryption),
            "original source encryption"
        );
        assert_eq!(
            row.get("sourceSetComplete"),
            Some(&json!(true)),
            "complete group sources"
        );
    }
    Ok(rows)
}
