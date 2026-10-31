// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx character package` end to end over a synthetic head rig, body, pieces and animations.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    HEAD_REGIONS, HEAD_VERTICES, MALE_RIG_HASH, TestAnimation, TestChf, TestEntry, TestSkeleton,
    TestTrack, armour_entries, check_glb, head_entries, head_rig,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

struct Run {
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

fn archive(dir: &Path, entries: Vec<TestEntry>) -> Result<PathBuf, Box<dyn Error>> {
    crate::chf_glb::archive(dir, entries)
}

fn chf(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let blend = |part: usize| {
        if part == 3 {
            vec![(65535, 2), (0, 0)]
        } else {
            vec![(0, 0), (0, 0)]
        }
    };
    let test = TestChf {
        dna_hashes: Some((MALE_RIG_HASH, 0)),
        face_parts: (0..HEAD_REGIONS).map(blend).collect(),
        ..TestChf::v8()
    };
    let path = dir.join("hero.chf");
    fs::write(&path, test.to_bytes())?;
    Ok(path)
}

fn animations() -> Vec<TestEntry> {
    let blink = TestAnimation::new("blink.caf", 30).track(
        TestTrack::new(crc32fast::hash(b"CTL_a")).positions(&[
            (0, [0.0, 0.0, 0.0]),
            (10, [1.0, 0.0, 0.0]),
            (20, [0.0, 0.0, 0.0]),
        ]),
    );
    let nod = TestAnimation::new("nod.caf", 30).track(
        TestTrack::new(TestSkeleton::controller_id(2))
            .rotations(&[(0, [0.0, 0.0, 0.0, 1.0]), (10, [0.0, 0.0, 0.0, 1.0])]),
    );
    vec![
        TestEntry::stored(
            "Data\\Animations\\Characters\\Human\\male_v7\\blink.caf",
            blink.caf(),
        ),
        TestEntry::stored(
            "Data\\Animations\\Characters\\Human\\male_v7\\nod.caf",
            nod.caf(),
        ),
    ]
}

fn package(archive: &Path, out: &Path, extra: &[&str], json: bool) -> Result<Run, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    let output = command
        .args(["character", "package"])
        .arg(archive)
        .arg("--out")
        .arg(out)
        .args(extra)
        .output()?;
    Ok(Run {
        stdout: String::from_utf8(output.stdout)?,
        stderr: String::from_utf8(output.stderr)?,
        code: output.status.code(),
    })
}

fn names(folder: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut names: Vec<String> = fs::read_dir(folder)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()?;
    names.sort();
    Ok(names)
}

fn manifest(out: &Path) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&fs::read(out.join("scene.json"))?)?)
}

fn full_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let rig = head_rig(HEAD_VERTICES, MALE_RIG_HASH).build();
    let mut entries = head_entries(Some(rig));
    entries.extend(animations());
    archive(dir, entries)
}

#[test]
fn a_character_with_animations_is_a_package_of_one_skinned_glb() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = full_archive(dir.path())?;
    let chf = chf(dir.path())?;
    let out = dir.path().join("hero");

    let run = package(
        &archive,
        &out,
        &[
            "--chf",
            chf.to_str().ok_or("path")?,
            "--anim",
            "all",
            "--materials",
            "colours",
        ],
        false,
    )?;

    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    assert_eq!(names(&out)?, ["meshes", "scene.json", "textures"]);
    assert_eq!(names(&out.join("meshes"))?, ["m0001.glb"]);
    assert!(run.stdout.contains("on the male body"), "{}", run.stdout);
    assert!(run.stdout.contains("2 animations"), "{}", run.stdout);

    let manifest = manifest(&out)?;
    assert_eq!(manifest["version"], "1.7");
    assert_eq!(manifest["nodes"][0]["kind"], "character");
    assert_eq!(manifest["nodes"][0]["name"], "hero");
    assert_eq!(manifest["nodes"][0]["mesh"], "m0001");
    assert_eq!(manifest["source"]["character"]["chf"], "hero.chf");
    assert_eq!(manifest["source"]["character"]["name"], "hero");
    let rig = &manifest["meshes"][0]["rig"];
    assert_eq!(rig["body"], "male");
    let head = rig["parts"]
        .as_array()
        .ok_or("parts")?
        .iter()
        .find(|part| part["name"] == "head")
        .ok_or("head part")?;
    assert!(
        head["morph_targets"]
            .as_u64()
            .is_some_and(|count| count > 0)
    );
    assert_eq!(rig["animations"].as_array().ok_or("clips")?.len(), 2);

    let glb = check_glb(&fs::read(out.join("meshes").join("m0001.glb"))?)?;
    assert_eq!(glb.document.skins().len(), 1);
    assert_eq!(glb.document.animations().count(), 2);
    let morphs = glb.document.meshes().any(|mesh| {
        mesh.primitives()
            .any(|primitive| primitive.morph_targets().len() > 0)
    });
    assert!(morphs, "the head carries morph targets");
    Ok(())
}

#[test]
fn pieces_on_a_chosen_body_are_a_package_and_the_json_report_names_the_parts() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), armour_entries())?;
    let out = dir.path().join("armour");

    let run = package(
        &archive,
        &out,
        &[
            "--piece",
            "TEST_Suit",
            "--body",
            "female",
            "--materials",
            "colours",
        ],
        true,
    )?;

    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    assert_eq!(run.stdout.lines().count(), 1, "one line of JSON");
    let report: Value = serde_json::from_str(&run.stdout)?;
    assert_eq!(report["body"], "female");
    assert_eq!(report["name"], "female");
    assert_eq!(report["bones"], 4);
    let origins: Vec<(&str, &str)> = report["parts"]
        .as_array()
        .ok_or("parts")?
        .iter()
        .filter_map(|part| Some((part["name"].as_str()?, part["origin"].as_str()?)))
        .collect();
    assert_eq!(origins, [("body", "body"), ("TEST_Suit", "piece")]);
    let manifest = manifest(&out)?;
    assert_eq!(manifest["meshes"][0]["rig"]["bones"], 4);
    assert_eq!(manifest["nodes"][0]["kind"], "character");
    assert_eq!(manifest["source"]["character"]["body"], "female");
    assert!(manifest["source"]["character"].get("chf").is_none());
    Ok(())
}

#[test]
fn an_output_inside_a_protected_installation_is_invalid_input_and_writes_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), armour_entries())?;
    let installation = dir.path().join("StarCitizen");
    fs::create_dir(&installation)?;
    let out = installation.join("LIVE").join("package");

    let output = p4kx_testkit::p4kx()?
        .env("P4KX_PROTECT_FOLDERS", &installation)
        .args(["character", "package"])
        .arg(&archive)
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("never writes there"),
        "{output:?}"
    );
    assert_eq!(names(&installation)?.len(), 0, "nothing was created");
    Ok(())
}

#[test]
fn a_piece_that_cannot_be_fitted_is_an_issue_of_the_manifest() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), armour_entries())?;
    let out = dir.path().join("armour");

    let run = package(
        &archive,
        &out,
        &[
            "--piece",
            "TEST_Boots",
            "--body",
            "female",
            "--materials",
            "colours",
        ],
        false,
    )?;

    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    assert!(
        run.stdout
            .contains("skipped TEST_Boots: the Entity has no geometry for the female body"),
        "{}",
        run.stdout
    );
    let manifest = manifest(&out)?;
    let issue = &manifest["issues"][0];
    assert_eq!(issue["skipped"], true);
    assert_eq!(issue["name"], "TEST_Boots");
    Ok(())
}

#[test]
fn the_chf_decides_the_body_so_a_body_beside_it_is_a_usage_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = full_archive(dir.path())?;
    let chf = chf(dir.path())?;
    let out = dir.path().join("hero");

    let run = package(
        &archive,
        &out,
        &["--chf", chf.to_str().ok_or("path")?, "--body", "male"],
        false,
    )?;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(!out.exists());
    Ok(())
}

#[test]
fn a_folder_that_is_not_a_package_is_refused_before_anything_is_read() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), armour_entries())?;
    let out = dir.path().join("mine");
    fs::create_dir(&out)?;
    fs::write(out.join("notes.txt"), b"mine")?;

    let run = package(&archive, &out, &[], false)?;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert_eq!(names(&out)?, ["notes.txt"]);
    Ok(())
}

#[test]
fn an_earlier_character_package_is_replaced() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), armour_entries())?;
    let out = dir.path().join("body");

    let first = package(&archive, &out, &["--materials", "colours"], false)?;
    let second = package(&archive, &out, &["--materials", "colours"], false)?;

    assert_eq!(first.code, Some(0), "{}", first.stderr);
    assert_eq!(second.code, Some(0), "{}", second.stderr);
    assert_eq!(names(dir.path())?, ["Data.p4k", "body"]);
    Ok(())
}
