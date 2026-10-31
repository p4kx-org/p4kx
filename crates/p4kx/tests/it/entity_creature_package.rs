// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx entity package` of a creature with its skeleton writes one skinned rig mesh and says so.

use std::error::Error;

use p4kx_testkit::{CREATURE, CREATURE_NO_SKELETON, creature_entries};
use serde_json::Value;

use crate::entity_package::{manifest, package, write_archive};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn a_creature_with_its_skeleton_is_one_rig_mesh_and_the_report_counts_the_bones() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), creature_entries())?;
    let out = dir.path().join("package");

    let run = package(&archive, CREATURE, &out, &[], false)?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    assert!(
        run.stdout
            .contains("rig: 3 bones, 1 skinned meshes in 1 file"),
        "{}",
        run.stdout
    );
    let manifest = manifest(&out)?;
    assert_eq!(manifest["version"], "1.7");
    assert_eq!(manifest["meshes"].as_array().map(Vec::len), Some(1));
    assert_eq!(manifest["meshes"][0]["rig"]["bones"], 3);
    assert!(manifest["meshes"][0]["rig"].get("body").is_none());
    assert_eq!(manifest["nodes"][0]["kind"], "entity");
    Ok(())
}

#[test]
fn the_json_report_counts_bones_and_skinned_meshes_only_for_a_rig() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), creature_entries())?;

    let rigged = package(&archive, CREATURE, &dir.path().join("rigged"), &[], true)?;
    let rigged: Value = serde_json::from_str(&rigged.stdout)?;
    assert_eq!(
        (&rigged["bones"], &rigged["skinned_meshes"]),
        (&3.into(), &1.into())
    );

    let plain = package(
        &archive,
        CREATURE_NO_SKELETON,
        &dir.path().join("plain"),
        &[],
        true,
    )?;
    let plain: Value = serde_json::from_str(&plain.stdout)?;
    assert!(plain.get("bones").is_none() && plain.get("skinned_meshes").is_none());
    assert!(manifest(&dir.path().join("plain"))?["meshes"][0]["rig"].is_null());
    Ok(())
}
