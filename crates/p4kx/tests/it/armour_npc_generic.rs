// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx armour glb --loadout` and `p4kx entity glb` for an NPC whose generic head names no body:
//! the `Char_Body` Entity names it, and a head without a model adds no piece.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, NPC_GENERIC_BODY_CLASS, NPC_GENERIC_CLASS, NPC_GENERIC_HEAD_CLASS,
    npc_generic_entries,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

fn archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    npc_generic_entries()
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(path)
}

fn run(args: &[&str], archive: &Path, out: &Path, json: bool) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    let output = command
        .args(args)
        .arg(archive)
        .args(["--out"])
        .arg(out)
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

#[test]
fn an_armour_glb_of_a_generic_npc_is_built_on_the_body_its_body_entity_names() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let out = dir.path().join("npc.glb");

    let stdout = run(
        &["armour", "glb", "--loadout", NPC_GENERIC_CLASS],
        &archive,
        &out,
        true,
    )?;

    let report: Value = serde_json::from_slice(&stdout)?;
    assert_eq!(report["body"], "female");
    assert_eq!(report["head"]["class_name"], NPC_GENERIC_HEAD_CLASS);
    assert_eq!(report["head"]["body"], "female");
    assert_eq!(report["head"]["parts"], 1);
    let issues = report["issues"].as_array().ok_or("no issues")?;
    assert!(
        issues
            .iter()
            .all(|issue| issue["kind"] != "no_worn_geometry"),
        "{issues:?}"
    );
    Ok(())
}

#[test]
fn an_entity_glb_of_a_generic_npc_says_the_body_entity_named_the_body() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let out = dir.path().join("npc.glb");

    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .args(["entity", "glb"])
        .arg(&archive)
        .arg(NPC_GENERIC_CLASS)
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["body"]["variant"], "female");
    assert_eq!(report["body"]["head"], NPC_GENERIC_HEAD_CLASS);
    assert_eq!(report["body"]["named_by"], NPC_GENERIC_BODY_CLASS);
    assert_eq!(report["body"]["named_by_head"], false);
    Ok(())
}

#[test]
fn the_text_report_of_a_generic_npc_names_the_body_entity() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let out = dir.path().join("npc.glb");

    let output = p4kx_testkit::p4kx()?
        .args(["entity", "glb"])
        .arg(&archive)
        .arg(NPC_GENERIC_CLASS)
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8_lossy(&output.stdout);
    let expected = format!("body female, named by the body {NPC_GENERIC_BODY_CLASS}");
    assert!(text.lines().any(|line| line == expected), "{text}");
    Ok(())
}
