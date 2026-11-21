// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx ships` end to end, over a synthetic DataForge and a vehicle implementation.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, entity_schema};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const XML: &str = r#"<Vehicle name="ACME_Alpha"><Parts>
  <Part name="hull" class="Animated" mass="1000" damageMax="500"/>
</Parts></Vehicle>"#;

fn archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    archive_with(dir, true)
}

/// The archive with the DataForge encrypted or not, and the vehicle XML always encrypted when the
/// DataForge is not, so that a run without a key can read the ships but not their parts.
fn archive_with(dir: &Path, forge_encrypted: bool) -> Result<PathBuf, Box<dyn Error>> {
    let ship = TestEntity::new(
        0x1,
        "ACME_Alpha",
        "libs/foundry/records/entities/spaceships/acme_alpha.xml",
    )
    .size(2)
    .vehicle(
        "Scripts/Entities/Vehicles/Implementations/Xml/ACME_Alpha.xml",
        "",
    );
    let other = TestEntity::new(
        0x2,
        "ACME_Beta",
        "libs/foundry/records/entities/spaceships/acme_beta.xml",
    );
    let database = entity_schema(DataForgeBuilder::new())
        .record(ship.record())
        .record(other.record());
    let path = dir.join("Data.p4k");
    let forge = TestEntry::zstd("Data\\Game2.dcb", database.build());
    let vehicle = TestEntry::zstd(
        "Data\\Scripts\\Entities\\Vehicles\\Implementations\\Xml\\ACME_Alpha.xml",
        XML.as_bytes().to_vec(),
    );
    let (forge, vehicle) = if forge_encrypted {
        (forge.encrypted(), vehicle)
    } else {
        (forge, vehicle.encrypted())
    };
    ArchiveBuilder::new()
        .entry(forge)
        .entry(vehicle)
        .write_to(&path)?;
    Ok(path)
}

fn run(archive: &Path, out: &Path, args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .arg("ships")
        .arg(archive)
        .arg("--out")
        .arg(out)
        .args(args)
        .env("SOURCE_DATE_EPOCH", "1792000000")
        .output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    Ok(String::from_utf8(output.stdout)?)
}

#[test]
fn ships_writes_one_json_document() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let out = dir.path().join("ships.json");
    let printed = run(&archive, &out, &[])?;
    assert!(
        printed.contains("2 ships (1 with mass, 1 with hull"),
        "{printed}"
    );
    let document: Value = serde_json::from_slice(&fs::read(&out)?)?;
    assert_eq!(document["schemaVersion"], 2);
    assert_eq!(
        document["source"]["exportedAt"].as_str().map(str::len),
        Some(20),
        "an RFC 3339 time from SOURCE_DATE_EPOCH"
    );
    let ships = document["ships"].as_array().ok_or("ships")?;
    assert_eq!(ships.len(), 2);
    assert_eq!(ships[0]["className"], "ACME_Alpha");
    assert_eq!(ships[0]["size"]["value"], 2);
    assert_eq!(ships[0]["mass"]["kg"], 1000.0);
    assert_eq!(ships[0]["hull"]["health"], 500.0);
    assert!(ships[1]["mass"].is_null() && ships[1]["hull"].is_null());
    Ok(())
}

#[test]
fn ships_writes_ndjson_and_a_json_summary() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let out = dir.path().join("ships.ndjson");
    let printed = run(
        &archive,
        &out,
        &["--format", "ndjson", "--filter", "acme_alpha", "--json"],
    )?;
    let summary: Value = serde_json::from_str(&printed)?;
    assert_eq!(summary["ships"], 1);
    assert_eq!(summary["format"], "ndjson");
    assert_eq!(summary["mass"], 1);
    assert_eq!(summary["archive_key"], "given");
    assert_eq!(summary["encrypted_entries_unread"], 0);
    let text = fs::read_to_string(&out)?;
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), 1);
    let ship: Value = serde_json::from_str(lines[0])?;
    assert_eq!(ship["className"], "ACME_Alpha");
    assert_eq!(
        ship["mass"]["sources"][0]["kind"], "vehicleXml",
        "each value names its source"
    );
    Ok(())
}

#[test]
fn the_json_summary_says_when_encrypted_entries_stayed_unread_without_a_key() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive_with(dir.path(), false)?;
    let out = dir.path().join("ships.json");

    let output = p4kx_testkit::p4kx()?
        .arg("ships")
        .arg(&archive)
        .arg("--out")
        .arg(&out)
        .arg("--json")
        .env_remove("P4KX_ARCHIVE_KEY")
        .env(
            "P4KX_KEY_FILE",
            dir.path().join("profile").join("archive.key"),
        )
        .output()?;

    let summary: Value =
        serde_json::from_slice(&output.stdout).map_err(|error| format!("{error}: {output:?}"))?;
    assert_eq!(summary["archive_key"], "missing", "{summary}");
    assert_eq!(summary["encrypted_entries_unread"], 1, "{summary}");
    let document: Value = serde_json::from_slice(&fs::read(&out)?)?;
    assert_eq!(document["source"]["archiveKey"], "missing");
    assert_eq!(document["source"]["encryptedEntriesUnread"], 1);
    Ok(())
}
