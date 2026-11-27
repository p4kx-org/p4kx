// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, TestChf, TestEntry};
use serde_json::{Value, json};

type Check = Result<(), Box<dyn std::error::Error>>;

fn command(archive: &Path, output: &Path) -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("p4kx"));
    command
        .args(["character", "source"])
        .arg(archive)
        .arg("--out")
        .arg(output);
    command
}

#[test]
fn source_cli_preserves_legacy_and_reserves_a_new_bundle_directory() -> Check {
    let folder = tempfile::tempdir()?;
    let archive = folder.path().join("source.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Libs/CharacterCustomizer/masculinedefault.chf",
            TestChf::v7().to_bytes(),
        ))
        .entry(TestEntry::stored(
            "Data/Libs/CharacterCustomizer/femininedefault.chf",
            TestChf::v8().to_bytes(),
        ))
        .write_to(&archive)?;
    let legacy = folder.path().join("source.json");
    command(&archive, &legacy)
        .args(["--body", "male"])
        .assert()
        .success();
    let original = fs::read(&legacy)?;
    let json: Value = serde_json::from_slice(&original)?;
    assert_eq!(json.get("schemaVersion"), Some(&json!(1)), "legacy schema");
    command(&archive, &legacy)
        .args(["--body", "male", "--body", "female"])
        .assert()
        .failure();
    assert_eq!(fs::read(&legacy)?, original, "legacy file preserved");
    let bundle = folder.path().join("bundle");
    command(&archive, &bundle)
        .args(["--bundle", "--body", "female", "--body", "male"])
        .assert()
        .success();
    let manifest = fs::read(bundle.join("manifest.json"))?;
    let json: Value = serde_json::from_slice(&manifest)?;
    assert_eq!(
        json.get("bodies"),
        Some(&json!(["female", "male"])),
        "explicit request order"
    );
    command(&archive, &bundle)
        .args(["--bundle", "--body", "male"])
        .assert()
        .failure();
    assert_eq!(
        fs::read(bundle.join("manifest.json"))?,
        manifest,
        "fresh-directory contract"
    );
    Ok(())
}
