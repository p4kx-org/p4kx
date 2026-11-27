// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx armour bodies` and `p4kx armour glb` end to end over the synthetic bodies and pieces of
//! the testkit.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{ArchiveBuilder, TestEntry, armour_entries, check_glb};

type TestResult = Result<(), Box<dyn Error>>;

struct Run {
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

fn write_archive(dir: &Path, entries: Vec<TestEntry>) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(path)
}

fn run(args: &[&str], archive: Option<&Path>, json: bool) -> Result<Run, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    command.arg("armour").args(args);
    if let Some(archive) = archive {
        command.arg(archive);
    }
    Ok(output(command.output()?)?)
}

fn output(output: std::process::Output) -> Result<Run, std::string::FromUtf8Error> {
    Ok(Run {
        stdout: String::from_utf8(output.stdout)?,
        stderr: String::from_utf8(output.stderr)?,
        code: output.status.code(),
    })
}

fn glb(
    archive: &Path,
    out: &Path,
    pieces: &[&str],
    extra: &[&str],
    json: bool,
) -> Result<Run, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    command.args(["armour", "glb"]).arg(archive);
    for piece in pieces {
        command.args(["--piece", piece]);
    }
    command.arg("--out").arg(out).args(extra);
    Ok(output(command.output()?)?)
}

#[test]
fn the_body_variants_are_listed() -> TestResult {
    let text = run(&["bodies"], None, false)?;
    assert_eq!(text.code, Some(0), "{}", text.stderr);
    assert!(
        text.stdout.contains("male  tag Male  skeleton "),
        "{}",
        text.stdout
    );
    assert!(
        text.stdout.contains("female  tag Female "),
        "{}",
        text.stdout
    );

    let json = run(&["bodies"], None, true)?;
    let value: serde_json::Value = serde_json::from_str(&json.stdout)?;
    let codes: Vec<&str> = value["bodies"]
        .as_array()
        .ok_or("bodies")?
        .iter()
        .filter_map(|body| body["code"].as_str())
        .collect();
    assert_eq!(codes, ["male", "female"]);
    Ok(())
}

#[test]
fn pieces_are_written_on_the_chosen_body_and_what_cannot_be_fitted_is_listed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), armour_entries())?;
    let out = dir.path().join("armour.glb");

    let run = glb(
        &archive,
        &out,
        &["TEST_Suit", "TEST_Boots", "TEST_Cape"],
        &["--body", "female"],
        false,
    )?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let checked = check_glb(&std::fs::read(&out)?)?;
    let skin = checked.document.skins().next().ok_or("a skin")?;
    assert_eq!(skin.joints().count(), 4, "the female skeleton");
    for expected in [
        "on the female body: 3 pieces, 2 parts (1 of the body), 4 bones",
        "part TEST_Suit: Objects/Characters/Human/female_v2/armor/test/f_suit.skin",
        "skipped TEST_Boots: the Entity has no geometry for the female body",
        "skipped TEST_Cape: the Entity names no geometry to wear",
    ] {
        assert!(run.stdout.contains(expected), "{expected}: {}", run.stdout);
    }
    Ok(())
}

#[test]
fn the_json_report_lists_parts_counts_and_issues() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), armour_entries())?;
    let out = dir.path().join("armour.glb");

    let run = glb(&archive, &out, &["TEST_Suit", "TEST_Plain"], &[], true)?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    assert_eq!(run.stdout.lines().count(), 1, "one line of JSON");
    let value: serde_json::Value = serde_json::from_str(&run.stdout)?;
    assert_eq!(value["body"], "male");
    assert_eq!(value["pieces"], 2);
    assert_eq!(value["bones"], 3);
    assert_eq!(value["skinned_meshes"], 3);
    assert_eq!(value["skipped"], 0);
    let origins: Vec<(&str, &str)> = value["parts"]
        .as_array()
        .ok_or("parts")?
        .iter()
        .filter_map(|part| Some((part["name"].as_str()?, part["origin"].as_str()?)))
        .collect();
    assert_eq!(
        origins,
        [
            ("body", "body"),
            ("TEST_Suit", "piece"),
            ("TEST_Plain", "piece")
        ]
    );
    let kinds: Vec<&str> = value["issues"]
        .as_array()
        .ok_or("issues")?
        .iter()
        .filter_map(|issue| issue["kind"].as_str())
        .collect();
    assert_eq!(kinds, ["bones_not_in_skeleton"]);
    Ok(())
}

#[test]
fn json_origins_keep_the_selected_loadout_separate_from_the_body_override() -> TestResult {
    use p4kx_testkit::test_key;
    use serde_json::Value;

    let mut payloads = Vec::new();
    for encrypted in [false, true] {
        let dir = tempfile::tempdir()?;
        let entries = armour_entries()
            .into_iter()
            .map(|entry| {
                if encrypted && entry.path() == "Data\\Game2.dcb" {
                    entry.encrypted()
                } else {
                    entry
                }
            })
            .collect();
        let archive = write_archive(dir.path(), entries)?;
        let out = dir.path().join("armour.glb");
        let key = test_key().to_hex();
        let result = glb(
            &archive,
            &out,
            &[],
            &[
                "--loadout",
                "TEST_Soldier",
                "--body",
                "female",
                "--key",
                &key,
            ],
            true,
        )?;
        assert_eq!(result.code, Some(0), "{}", result.stderr);
        let report: Value = serde_json::from_str(&result.stdout)?;
        check_json_origins(&report, encrypted)?;
        payloads.push(std::fs::read(out)?);
    }
    assert_eq!(
        payloads.first(),
        payloads.get(1),
        "Archive encryption does not change the GLB"
    );
    Ok(())
}

fn check_json_origins(report: &serde_json::Value, encrypted: bool) -> TestResult {
    use serde_json::Value;

    check_loadout_class_origin(report, encrypted)?;
    let fields = &report["fieldOrigins"];
    assert_eq!(
        fields.get("/parts/1/file").ok_or("worn model selector")?["containsEncryptedSource"],
        if encrypted {
            Value::Bool(true)
        } else {
            Value::Null
        },
        "the selected geometry field contributes while model selection remains partial"
    );
    assert_eq!(report["body"], "female", "the explicit body override wins");
    let body = fields.get("/body").ok_or("body origin")?;
    assert_eq!(
        body["containsEncryptedSource"], false,
        "the override is tool-authored"
    );
    assert_eq!(
        body["sourceSetComplete"], true,
        "the override has complete inputs"
    );
    let bones = fields.get("/bones").ok_or("bone count origin")?;
    assert!(
        !bones["sourceUsesRefs"]
            .as_array()
            .ok_or("rig uses")?
            .is_empty(),
        "the emitted bones include the physically read base rig"
    );
    let nodes = fields.get("/nodes").ok_or("node count origin")?;
    assert_eq!(
        nodes["containsEncryptedSource"],
        Value::Null,
        "other node inputs remain unknown"
    );
    assert_eq!(
        nodes["sourceSetComplete"], false,
        "the base rig alone does not prove this count"
    );
    Ok(())
}

fn check_loadout_class_origin(report: &serde_json::Value, encrypted: bool) -> TestResult {
    use serde_json::Value;

    let class = report["fieldOrigins"]
        .get("/loadout/class_name")
        .ok_or("loadout class origin")?;
    assert_eq!(
        report
            .pointer("/loadout/class_name")
            .and_then(Value::as_str),
        Some("TEST_Soldier"),
        "the original selected class"
    );
    assert_eq!(
        class["containsEncryptedSource"], encrypted,
        "DataForge origin"
    );
    assert_eq!(
        class["sourceSetComplete"], true,
        "complete class name inputs"
    );
    let refs = class["sourceUsesRefs"].as_array().ok_or("class uses")?;
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    assert!(
        uses.iter().any(|usage| {
            usage.get("id").is_some_and(|id| refs.contains(id))
                && usage
                    .pointer("/location/recordIndex")
                    .and_then(Value::as_u64)
                    == Some(12)
                && usage.pointer("/location/field").and_then(Value::as_str) == Some("recordName")
        }),
        "the class evidence identifies the TEST_Soldier fixture Record"
    );
    Ok(())
}

#[test]
fn the_bare_body_is_written_without_pieces() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), armour_entries())?;
    let out = dir.path().join("body.glb");

    let run = glb(&archive, &out, &[], &[], false)?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let checked = check_glb(&std::fs::read(&out)?)?;
    assert_eq!(checked.document.skins().len(), 1);
    Ok(())
}

#[test]
fn an_unknown_piece_is_invalid_input_and_writes_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), armour_entries())?;
    let out = dir.path().join("armour.glb");

    let run = glb(&archive, &out, &["TEST_Nothing"], &[], false)?;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(run.stderr.contains("TEST_Nothing"), "{}", run.stderr);
    assert!(!out.exists(), "no output");
    Ok(())
}

#[test]
fn an_archive_without_the_skeleton_is_an_archive_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), armour_entries().into_iter().take(1).collect())?;
    let out = dir.path().join("armour.glb");

    let run = glb(&archive, &out, &["TEST_Suit"], &[], false)?;

    assert_eq!(run.code, Some(3), "{}", run.stderr);
    assert!(run.stderr.contains("skeleton"), "{}", run.stderr);
    Ok(())
}

#[test]
fn an_output_that_is_the_archive_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), armour_entries())?;
    let before = std::fs::read(&archive)?;

    let run = glb(&archive, &archive, &["TEST_Suit"], &[], false)?;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert_eq!(std::fs::read(&archive)?, before, "the Archive is untouched");
    Ok(())
}
