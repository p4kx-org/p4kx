// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `--loadout-pick` of `p4kx entity loadout`, `entity glb` and `entity package` chooses the
//! alternative of a group of a generated Loadout.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, assembly_entries,
    assortment_record, entity_schema,
};

use crate::entity_package::{package, write_archive};

type TestResult = Result<(), Box<dyn Error>>;

const ASSORTMENT: u128 = 0x100;
const BEAST: &str = "TEST_Beast";

fn on_relay(class: &str) -> TestLoadout {
    TestLoadout::Manual(vec![TestLoadoutEntry::class("hardpoint_relay", class)])
}

/// An archive of the sample assembly models whose `TEST_Beast` has a model and a generated
/// Loadout: the group `Body` puts `TEST_Gun` or `TEST_Mount` on `hardpoint_relay`, the group
/// `Eggs` has one alternative.
fn archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let forge = [
        TestEntity::new(0x10, BEAST, "entities/spaceships/test_beast.xml")
            .geometry("Objects\\Test\\ship.cgf", None)
            .loadout(TestLoadout::RandomGenerated(vec![ASSORTMENT])),
        TestEntity::new(0x11, "TEST_Gun", "entities/scitem/test_gun.xml")
            .geometry("Objects/Test/gun.cgf", None),
        TestEntity::new(0x12, "TEST_Mount", "entities/scitem/test_mount.xml")
            .geometry("Objects/Test/mount.cgf", None),
        TestEntity::new(0x13, "TEST_Egg", "entities/scitem/test_egg.xml"),
    ]
    .iter()
    .fold(entity_schema(DataForgeBuilder::new()), |forge, entity| {
        forge.record(entity.record())
    })
    .record(assortment_record(
        ASSORTMENT,
        "beast",
        &[
            ("Body", vec![on_relay("TEST_Gun"), on_relay("TEST_Mount")]),
            (
                "Eggs",
                vec![TestLoadout::Manual(vec![TestLoadoutEntry::class(
                    "egg_port", "TEST_Egg",
                )])],
            ),
        ],
    ));
    let mut entries = assembly_entries();
    entries.retain(|entry| !entry.path().ends_with("Game2.dcb"));
    entries.push(TestEntry::zstd("Data\\Game2.dcb", forge.build()));
    write_archive(dir, entries)
}

struct Run {
    stdout: String,
    code: Option<i32>,
}

fn export(action: &str, archive: &Path, extra: &[&str]) -> Result<Run, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .args(["entity", action])
        .arg(archive)
        .arg(BEAST)
        .args(extra)
        .output()?;
    Ok(Run {
        stdout: String::from_utf8(output.stdout)?,
        code: output.status.code(),
    })
}

fn loadout(archive: &Path, picks: &[&str]) -> Result<Run, Box<dyn Error>> {
    let extra: Vec<&str> = picks
        .iter()
        .flat_map(|pick| ["--loadout-pick", pick])
        .collect();
    export("loadout", archive, &extra)
}

fn attached(run: &Run) -> (bool, bool) {
    (
        run.stdout.contains("TEST_Gun"),
        run.stdout.contains("TEST_Mount"),
    )
}

#[test]
fn without_a_pick_the_first_alternative_is_attached() -> TestResult {
    let dir = tempfile::tempdir()?;
    let run = loadout(&archive(dir.path())?, &[])?;

    assert_eq!(run.code, Some(0));
    assert_eq!(attached(&run), (true, false), "{}", run.stdout);
    Ok(())
}

#[test]
fn a_named_pick_attaches_the_other_alternative() -> TestResult {
    let dir = tempfile::tempdir()?;
    let run = loadout(&archive(dir.path())?, &["body=1"])?;

    assert_eq!(run.code, Some(0));
    assert_eq!(attached(&run), (false, true), "{}", run.stdout);
    Ok(())
}

#[test]
fn commas_separate_picks_like_several_options() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;

    let commas = loadout(&archive, &["Eggs=0,Body=1"])?;
    let options = loadout(&archive, &["Eggs=0", "Body=1"])?;

    assert_eq!(commas.stdout, options.stdout);
    assert_eq!(attached(&commas), (false, true));
    Ok(())
}

#[test]
fn an_index_beyond_the_group_is_listed_and_the_first_stays() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;

    for index in ["7", "99999999999"] {
        let run = loadout(&archive, &[&format!("Body={index}")])?;

        assert_eq!(run.code, Some(0), "{index}");
        assert!(run.stdout.contains("beyond"), "{}", run.stdout);
        assert_eq!(attached(&run), (true, false), "{}", run.stdout);
    }
    Ok(())
}

#[test]
fn a_named_group_the_loadout_lacks_is_listed_and_the_export_succeeds() -> TestResult {
    let dir = tempfile::tempdir()?;
    let run = loadout(&archive(dir.path())?, &["Bodyy=2"])?;

    assert_eq!(run.code, Some(0));
    assert!(run.stdout.contains("Bodyy=2"), "{}", run.stdout);
    assert_eq!(attached(&run), (true, false), "{}", run.stdout);
    Ok(())
}

#[test]
fn a_bare_index_beyond_every_group_is_silent() -> TestResult {
    let dir = tempfile::tempdir()?;
    let run = loadout(&archive(dir.path())?, &["9"])?;

    assert_eq!(run.code, Some(0));
    assert!(!run.stdout.contains("! "), "{}", run.stdout);
    assert_eq!(attached(&run), (true, false), "{}", run.stdout);
    Ok(())
}

#[test]
fn a_malformed_pick_is_invalid_input_before_the_archive_is_read() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;

    for pick in ["Body=", "=2", "Body=first", "Body=+1", "+1"] {
        assert_eq!(loadout(&archive, &[pick])?.code, Some(2), "{pick}");
    }
    let output = p4kx_testkit::p4kx()?
        .args(["entity", "loadout"])
        .arg(dir.path().join("missing.p4k"))
        .arg(BEAST)
        .arg("--loadout-pick=-1")
        .output()?;
    let stderr = String::from_utf8(output.stderr)?;
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("-1"),
        "the pick is named, not the archive: {stderr}"
    );
    Ok(())
}

#[test]
fn entity_glb_places_the_picked_alternative() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;

    let glb_of = |picks: &[&str]| -> Result<String, Box<dyn Error>> {
        let out = dir.path().join(format!("beast{}.glb", picks.len()));
        let mut extra = vec!["--out", out.to_str().ok_or("path")?];
        extra.extend(picks.iter().flat_map(|pick| ["--loadout-pick", pick]));
        let run = export("glb", &archive, &extra)?;
        assert_eq!(run.code, Some(0), "{}", run.stdout);
        Ok(String::from_utf8_lossy(&std::fs::read(&out)?).into_owned())
    };
    let first = glb_of(&[])?;
    let second = glb_of(&["Body=1"])?;

    assert!(first.contains("TEST_Gun") && !first.contains("TEST_Mount"));
    assert!(second.contains("TEST_Mount") && !second.contains("TEST_Gun"));
    Ok(())
}

#[test]
fn entity_package_places_the_picked_alternative() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;

    let scene_of = |name: &str, extra: &[&str]| -> Result<String, Box<dyn Error>> {
        let out = dir.path().join(name);
        let run = package(&archive, BEAST, &out, extra, false)?;
        assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
        Ok(std::fs::read_to_string(out.join("scene.json"))?)
    };
    let first = scene_of("first", &[])?;
    let second = scene_of("second", &["--loadout-pick", "Body=1"])?;

    assert!(first.contains("TEST_Gun") && !first.contains("TEST_Mount"));
    assert!(second.contains("TEST_Mount") && !second.contains("TEST_Gun"));
    Ok(())
}
