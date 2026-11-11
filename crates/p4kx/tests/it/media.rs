// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_testkit::{ArchiveBuilder, TestCodec, TestEntry, TestWem};
use predicates::prelude::predicate;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn media_export_runs_the_permanent_provider_and_emits_the_1_0_contract() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("a.bin", b"first"))
        .entry(TestEntry::stored("a.bin", b"second"))
        .write_to(&source)?;
    let output = folder.path().join("media");
    p4kx_testkit::p4kx()?
        .args(["media", "export"])
        .arg(&source)
        .arg("--out")
        .arg(&output)
        .args(["--kind", "archive", "--json"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("\"format_version\":\"1.0\""));
    let index: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("media-index.json"))?)?;
    assert_eq!(
        index.pointer("/format").and_then(serde_json::Value::as_str),
        Some("p4kx-media"),
        "the CLI writes the versioned media envelope"
    );
    assert_eq!(
        index
            .pointer("/source_entries/written")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "duplicate source paths retain two physical Entries"
    );
    Ok(())
}

#[test]
fn explicit_unavailable_kind_is_non_success_and_writes_no_root_index() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    ArchiveBuilder::new().write_to(&source)?;
    let output = folder.path().join("media");
    p4kx_testkit::p4kx()?
        .args(["media", "export"])
        .arg(&source)
        .arg("--out")
        .arg(&output)
        .args(["--kind", "model"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "media capability model is unavailable",
        ));
    assert!(
        !output.join("media-index.json").exists(),
        "an unavailable requested provider publishes no complete root"
    );
    Ok(())
}

#[test]
fn media_output_inside_the_opened_installation_is_refused() -> TestResult {
    let folder = tempfile::tempdir()?;
    let channel = folder.path().join("StarCitizen/PTU");
    fs::create_dir_all(channel.join("Bin64"))?;
    let source = channel.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("a", b"a"))
        .write_to(&source)?;
    let output = channel.join("export");
    p4kx_testkit::p4kx()?
        .args(["media", "export"])
        .arg(&source)
        .arg("--out")
        .arg(&output)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Star Citizen installation"));
    assert!(
        !output.exists(),
        "protected installation output is refused before creating a destination"
    );
    Ok(())
}

#[test]
fn original_missing_key_error_survives_media_source_and_extraction_wrappers() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("secret.bin", b"secret").encrypted())
        .write_to(&source)?;
    p4kx_testkit::p4kx()?
        .env_remove("P4KX_ARCHIVE_KEY")
        .arg("--key-file")
        .arg(folder.path().join("no-key.toml"))
        .args(["media", "export"])
        .arg(&source)
        .arg("--out")
        .arg(folder.path().join("media"))
        .assert()
        .code(7)
        .stderr(predicate::str::contains("P4KX_ARCHIVE_KEY"));
    Ok(())
}

#[test]
fn media_schemas_are_valid_json() -> TestResult {
    for arguments in [
        vec!["media", "schema"],
        vec!["media", "schema", "--record"],
        vec!["media", "schema", "--reference"],
        vec!["media", "schema", "--source"],
    ] {
        let result = p4kx_testkit::p4kx()?
            .args(arguments)
            .assert()
            .code(0)
            .get_output()
            .stdout
            .clone();
        let schema: serde_json::Value = serde_json::from_slice(&result)?;
        jsonschema::validator_for(&schema)?;
    }
    Ok(())
}

#[test]
fn zero_or_excessive_media_limits_are_invalid_input() -> TestResult {
    p4kx_testkit::p4kx()?
        .args([
            "media",
            "export",
            "unused.p4k",
            "--out",
            "unused",
            "--max-entry-bytes",
            "0",
        ])
        .assert()
        .code(2);
    p4kx_testkit::p4kx()?
        .args([
            "media",
            "export",
            "unused.p4k",
            "--out",
            "unused",
            "--max-container-depth",
            "9",
        ])
        .assert()
        .code(2);
    Ok(())
}

#[test]
fn media_cli_audio_emits_typed_wem_summary_and_directory() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    let wem = TestWem::pcm(TestCodec::Pcm, 1, 22_050, &[0, 1]).bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Audio/fixture.wem", wem))
        .write_to(&source)?;
    let output = folder.path().join("media");
    p4kx_testkit::p4kx()?
        .args(["media", "export"])
        .arg(&source)
        .arg("--out")
        .arg(&output)
        .args(["--kind", "audio"])
        .assert()
        .code(0);
    let index: p4kx_media::Index =
        serde_json::from_slice(&fs::read(output.join("media-index.json"))?)?;
    let schema = p4kx_testkit::p4kx()?
        .args(["media", "schema", "--record"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let validator =
        jsonschema::validator_for(&serde_json::from_slice::<serde_json::Value>(&schema)?)?;
    let stream = fs::read_to_string(output.join(&index.artifact_base).join("audio.jsonl"))?;
    let mut types = Vec::new();
    for line in stream.lines() {
        let row: serde_json::Value = serde_json::from_str(line)?;
        validator
            .validate(&row)
            .map_err(|error| error.to_string())?;
        let record_type = row
            .pointer("/data/record_type")
            .and_then(serde_json::Value::as_str)
            .ok_or("built-in Audio record discriminator")?;
        types.push(record_type.to_owned());
    }
    assert_eq!(
        types,
        ["wem", "wem-chunk", "wem-chunk"],
        "format and sample-data chunks follow the WEM header"
    );
    Ok(())
}
