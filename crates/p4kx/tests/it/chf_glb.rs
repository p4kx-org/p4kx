// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx chf glb` end to end over a synthetic head rig, body and CHF files.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::gltf::Semantic;
use p4kx_testkit::{
    ArchiveBuilder, HEAD_REGIONS, HEAD_VERTICES, MALE_BODY_TYPE, MALE_RIG_HASH, TestChf, TestEntry,
    check_glb, head_entries, head_rig,
};

#[path = "chf_glb_origins.rs"]
mod origins;

type TestResult = Result<(), Box<dyn Error>>;

pub(crate) struct Run {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) code: Option<i32>,
}

pub(crate) fn archive(dir: &Path, entries: Vec<TestEntry>) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(path)
}

fn chf(dir: &Path, name: &str, dna: Option<(u32, u16)>) -> Result<PathBuf, Box<dyn Error>> {
    let blend = |part: usize| match dna {
        Some((_, head)) if part == 3 => vec![(65535, head), (0, 0)],
        _ => vec![(0, 0), (0, 0)],
    };
    let test = TestChf {
        body_type: MALE_BODY_TYPE,
        dna_hashes: dna.map(|(hash, _)| (hash, 0)),
        face_parts: (0..HEAD_REGIONS).map(blend).collect(),
        ..TestChf::v8()
    };
    let path = dir.join(name);
    fs::write(&path, test.to_bytes())?;
    Ok(path)
}

pub(crate) fn glb(
    archive: &Path,
    chf: &Path,
    out: &Path,
    json: bool,
) -> Result<Run, Box<dyn Error>> {
    glb_with(archive, chf, out, json, &[])
}

pub(super) fn glb_with(
    archive: &Path,
    chf: &Path,
    out: &Path,
    json: bool,
    extra: &[&str],
) -> Result<Run, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    command
        .args(["chf", "glb"])
        .arg(archive)
        .arg(chf)
        .arg("--out")
        .arg(out)
        .args(extra);
    let output = command.output()?;
    Ok(Run {
        stdout: String::from_utf8(output.stdout)?,
        stderr: String::from_utf8(output.stderr)?,
        code: output.status.code(),
    })
}

fn head_positions(path: &Path) -> Result<Vec<f32>, Box<dyn Error>> {
    let glb = check_glb(&fs::read(path)?)?;
    let mesh = glb
        .document
        .nodes()
        .find(|node| node.name() == Some("head") && node.mesh().is_some())
        .and_then(|node| node.mesh())
        .ok_or("the head node")?;
    let accessor = mesh
        .primitives()
        .next()
        .and_then(|primitive| primitive.get(&Semantic::Positions))
        .ok_or("positions")?;
    Ok(glb
        .bytes(&accessor)
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&bytes| f32::from_le_bytes(bytes))
        .collect())
}

#[test]
fn a_character_is_written_as_its_head_on_the_body_of_its_variant() -> TestResult {
    let dir = tempfile::tempdir()?;
    let rig = head_rig(HEAD_VERTICES, MALE_RIG_HASH).build();
    let archive = archive(dir.path(), head_entries(Some(rig)))?;
    let shaped = chf(dir.path(), "shaped.chf", Some((MALE_RIG_HASH, 2)))?;
    let plain = chf(dir.path(), "plain.chf", Some((MALE_RIG_HASH, 0)))?;
    let (shaped_out, plain_out) = (dir.path().join("shaped.glb"), dir.path().join("plain.glb"));

    let first = glb(&archive, &shaped, &shaped_out, false)?;
    let second = glb(&archive, &plain, &plain_out, false)?;

    assert_eq!(first.code, Some(0), "{}", first.stderr);
    assert_eq!(second.code, Some(0), "{}", second.stderr);
    assert!(
        first
            .stdout
            .contains("the head of protos_human_male_face_t1_pu on the male body"),
        "{}",
        first.stdout
    );
    assert!(first.stdout.contains("part head: "), "{}", first.stdout);
    let (moved, base) = (head_positions(&shaped_out)?, head_positions(&plain_out)?);
    assert_eq!(moved.len(), base.len());
    let changed = moved
        .iter()
        .zip(&base)
        .filter(|(moved, base)| (*moved - *base).abs() > 1e-6)
        .count();
    assert_eq!(changed, 3, "one vertex moved on each axis");
    Ok(())
}

#[test]
fn the_json_report_names_the_rig_the_body_and_the_parts() -> TestResult {
    let dir = tempfile::tempdir()?;
    let rig = head_rig(HEAD_VERTICES, MALE_RIG_HASH).build();
    let archive = archive(dir.path(), head_entries(Some(rig)))?;
    let file = chf(dir.path(), "c.chf", Some((MALE_RIG_HASH, 2)))?;

    let run = glb(&archive, &file, &dir.path().join("c.glb"), true)?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let value: serde_json::Value = serde_json::from_str(&run.stdout)?;
    assert_eq!(value["body"], "male");
    assert_eq!(value["rig"], "protos_human_male_face_t1_pu");
    assert_eq!(value["skipped"], 0);
    let origins: Vec<&str> = value["parts"]
        .as_array()
        .ok_or("parts")?
        .iter()
        .filter_map(|part| part["origin"].as_str())
        .collect();
    assert_eq!(origins, ["body", "head", "head"]);
    let names: Vec<&str> = value["parts"]
        .as_array()
        .ok_or("parts")?
        .iter()
        .filter_map(|part| part["name"].as_str())
        .collect();
    assert_eq!(names, ["body", "head", "teeth"]);
    Ok(())
}

#[test]
fn a_head_whose_rig_is_missing_is_reported_as_skipped_and_exits_with_one() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), head_entries(None))?;
    let file = chf(dir.path(), "c.chf", Some((MALE_RIG_HASH, 2)))?;
    let out = dir.path().join("c.glb");

    let run = glb(&archive, &file, &out, false)?;
    let json = glb(&archive, &file, &out, true)?;

    assert_eq!(run.code, Some(1), "{}", run.stderr);
    assert!(run.stdout.contains("skipped head: "), "{}", run.stdout);
    assert!(run.stdout.contains("does not exist"), "{}", run.stdout);
    assert!(run.stderr.contains("could not be shaped"), "{}", run.stderr);
    assert!(out.exists(), "the GLB is written without the head");
    assert_eq!(json.code, Some(1), "{}", json.stderr);
    let value: serde_json::Value = serde_json::from_str(&json.stdout)?;
    assert_eq!(value["skipped"], 2, "the head and the teeth");
    assert_eq!(value["issues"][0]["kind"], "shape_failed");
    assert_eq!(
        value["fieldOrigins"]["/issues/0/kind"]["sourceSetComplete"],
        false
    );
    assert!(out.with_file_name("c.glb.p4kx-origin.jsonl").is_file());
    Ok(())
}

#[test]
fn a_character_without_face_dna_or_for_an_unknown_rig_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), head_entries(None))?;
    for (name, dna, message) in [
        ("none.chf", None, "no face DNA"),
        ("other.chf", Some((0xdead_beef, 1)), "unknown head rig"),
    ] {
        let file = chf(dir.path(), name, dna)?;

        let run = glb(&archive, &file, &dir.path().join("c.glb"), false)?;

        assert_eq!(run.code, Some(2), "{name}: {}", run.stderr);
        assert!(run.stderr.contains(message), "{name}: {}", run.stderr);
    }
    Ok(())
}

#[test]
fn a_file_that_is_no_chf_is_an_input_error_and_the_archive_is_never_the_output() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), head_entries(None))?;
    let junk = dir.path().join("junk.chf");
    fs::write(&junk, b"not a character")?;
    let file = chf(dir.path(), "c.chf", Some((MALE_RIG_HASH, 1)))?;

    let bad = glb(&archive, &junk, &dir.path().join("c.glb"), false)?;
    let same = glb(&archive, &file, &archive, false)?;

    assert_eq!(bad.code, Some(3), "{}", bad.stderr);
    assert_eq!(same.code, Some(2), "{}", same.stderr);
    Ok(())
}

fn target_names(path: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let glb = check_glb(&fs::read(path)?)?;
    let mesh = glb
        .document
        .nodes()
        .find(|node| node.name() == Some("head") && node.mesh().is_some())
        .and_then(|node| node.mesh())
        .ok_or("the head node")?;
    Ok(glb.target_names(mesh.index()).unwrap_or_default())
}

#[test]
fn the_expressions_are_written_by_default_limited_by_name_or_left_out() -> TestResult {
    let dir = tempfile::tempdir()?;
    let rig = head_rig(HEAD_VERTICES, MALE_RIG_HASH).build();
    let archive = archive(dir.path(), head_entries(Some(rig)))?;
    let file = chf(dir.path(), "c.chf", Some((MALE_RIG_HASH, 2)))?;
    let out = dir.path().join("c.glb");

    let all = glb_with(&archive, &file, &out, true, &[])?;
    let all_names = target_names(&out)?;
    let one = glb_with(&archive, &file, &out, false, &["--expression", "ctl_b"])?;
    let one_names = target_names(&out)?;
    let none = glb_with(&archive, &file, &out, false, &["--no-expressions"])?;
    let none_names = target_names(&out)?;

    assert_eq!(all.code, Some(0), "{}", all.stderr);
    assert_eq!(all_names, ["CTL_a", "CTL_b"]);
    let value: serde_json::Value = serde_json::from_str(&all.stdout)?;
    assert_eq!(value["morph_targets"], 2);
    assert_eq!(value["parts"][1]["morph_targets"], 2);
    assert_eq!(one.code, Some(0), "{}", one.stderr);
    assert_eq!(one_names, ["CTL_b"]);
    assert!(one.stdout.contains("1 morph targets"), "{}", one.stdout);
    assert_eq!(none.code, Some(0), "{}", none.stderr);
    assert_eq!(none_names.len(), 0);
    Ok(())
}

#[test]
fn an_expression_the_rig_lacks_is_listed_and_exits_with_one() -> TestResult {
    let dir = tempfile::tempdir()?;
    let rig = head_rig(HEAD_VERTICES, MALE_RIG_HASH).build();
    let archive = archive(dir.path(), head_entries(Some(rig)))?;
    let file = chf(dir.path(), "c.chf", Some((MALE_RIG_HASH, 2)))?;
    let out = dir.path().join("c.glb");

    let run = glb_with(
        &archive,
        &file,
        &out,
        false,
        &["--expression", "CTL_a", "--expression", "nope"],
    )?;

    assert_eq!(run.code, Some(1), "{}", run.stderr);
    assert!(run.stdout.contains("nope"), "{}", run.stdout);
    assert!(run.stderr.contains("expressions"), "{}", run.stderr);
    assert_eq!(target_names(&out)?, ["CTL_a"], "the file is still written");
    Ok(())
}

#[test]
fn no_expressions_and_a_chosen_expression_contradict_each_other() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), head_entries(None))?;
    let file = chf(dir.path(), "c.chf", Some((MALE_RIG_HASH, 2)))?;

    let run = glb_with(
        &archive,
        &file,
        &dir.path().join("c.glb"),
        false,
        &["--no-expressions", "--expression", "CTL_a"],
    )?;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    Ok(())
}
