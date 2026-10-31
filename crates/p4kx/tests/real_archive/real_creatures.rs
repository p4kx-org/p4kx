// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `entity glb` and `entity package` of Entities whose geometry is a skeleton file (`.chr`)
//! outside the human rigs. The Entities are picked from the Archive by structure: the first, by
//! class name, whose skeleton file the Archive holds and that draws without a skipped part, and
//! the first whose skeleton file the Archive lacks.

#![expect(
    clippy::print_stdout,
    reason = "the picks and measurements are the output"
)]

use std::error::Error;
use std::path::Path;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, ENTITY_TYPE, ForgePalettes,
    LoadoutResolver,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{check_glb, real_archive_key, real_archive_path};
use serde_json::Value;

use super::package_support::{Scratch, TestResult, build, verify};

/// A creature Entity picked from the Archive.
struct Pick {
    class: String,
    geometry: String,
}

/// The first Entities, by class name, that a test of each kind exports.
#[derive(Default)]
struct Picks {
    drawn: Option<Pick>,
    absent: Option<Pick>,
}

/// Scans the Entities whose geometry is a character file outside the human rigs.
fn pick() -> Result<Picks, Box<dyn Error>> {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let models = ArchiveModels::new(&archive);
    let palettes = ForgePalettes::new(&forge);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_files(&files)
        .stopping_at_docked_vehicles();
    let mut candidates: Vec<_> = forge
        .records()
        .filter(|record| record.type_name().is_ok_and(|name| name == ENTITY_TYPE))
        .filter_map(|record| {
            let tree = resolver.resolve(&record).ok()?;
            let geometry = tree
                .geometry()
                .filter(|geometry| geometry.is_rigged_file_outside_human_rigs())?;
            let path = geometry.path().to_owned();
            Some((tree, path))
        })
        .collect();
    candidates.sort_by_cached_key(|(tree, _)| tree.class_name().to_ascii_lowercase());
    let mut picks = Picks::default();
    for (tree, geometry) in candidates {
        if picks.drawn.is_some() && picks.absent.is_some() {
            break;
        }
        let present = models.contains(&geometry);
        let taken = if present { &picks.drawn } else { &picks.absent };
        if taken.is_some() || !geometry.to_ascii_lowercase().ends_with(".chr") {
            continue;
        }
        let assembly =
            Assembly::build_with_palettes(&tree, &models, &palettes, &AssemblyOptions::default())?;
        let pick = Pick {
            class: tree.class_name().to_owned(),
            geometry,
        };
        if present {
            if draws(&assembly) && skipped(&assembly) == 0 {
                picks.drawn = Some(pick);
            }
        } else if !draws(&assembly) {
            picks.absent = Some(pick);
        }
    }
    Ok(picks)
}

/// Returns whether any part of the Assembly has vertices to draw.
fn draws(assembly: &Assembly) -> bool {
    assembly.parts().iter().any(|part| {
        part.model()
            .is_some_and(|model| model.lods.iter().any(|lod| lod.vertex_count() > 0))
    })
}

fn skipped(assembly: &Assembly) -> usize {
    assembly
        .issues()
        .iter()
        .filter(|issue| issue.kind().skips())
        .count()
}

/// Runs `p4kx --json entity glb`, which must exit with 0, and returns the report and the GLB.
fn glb(class: &str, out: &Path) -> Result<(Value, Vec<u8>), Box<dyn Error>> {
    let mut run = p4kx_testkit::p4kx()?;
    run.env("P4KX_ARCHIVE_KEY", real_archive_key()?.to_hex());
    let output = run
        .args(["--json", "entity", "glb"])
        .arg(real_archive_path()?)
        .arg(class)
        .arg("--out")
        .arg(out)
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "{class}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = serde_json::from_slice(&output.stdout)?;
    Ok((report, std::fs::read(out)?))
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_creature_with_its_skeleton_exports_to_a_valid_glb_and_package() -> TestResult {
    let Some(pick) = pick()?.drawn else {
        return Err("the Archive holds no creature Entity that draws".into());
    };
    let scratch = Scratch::new("creature-glb")?;
    let out = scratch.path().join("creature.glb");

    let (report, bytes) = glb(&pick.class, &out)?;

    let checked = check_glb(&bytes)?;
    println!(
        "{} ({}): {} meshes, {} bytes",
        pick.class,
        pick.geometry,
        checked.document.meshes().count(),
        bytes.len()
    );
    assert!(checked.document.meshes().count() > 0, "{}", pick.class);
    assert!(report["modelled_parts"].as_u64().unwrap_or_default() > 0);
    assert_eq!(report["skipped"].as_u64(), Some(0), "{}", pick.class);

    let built = build(&scratch, "entity", Some(&pick.class), "package", &[])?;
    let measured = verify(&built)?;
    assert!(measured.meshes > 0, "{}: package meshes", pick.class);
    assert_eq!(measured.meshes, 1, "{}: one rig mesh", pick.class);
    let rig = &built.manifest["meshes"][0]["rig"];
    assert!(
        rig["bones"].as_u64().unwrap_or_default() > 0,
        "{}",
        pick.class
    );
    assert!(
        rig.get("body").is_none(),
        "{}: a creature has no body",
        pick.class
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_creature_whose_geometry_the_archive_lacks_fails_with_exit_code_2_naming_the_file() -> TestResult
{
    let Some(pick) = pick()?.absent else {
        return Err(
            "the Archive holds no skeleton-rooted Entity whose skeleton file is missing".into(),
        );
    };
    let scratch = Scratch::new("creature-absent")?;
    let out = scratch.path().join("creature.glb");
    let package = scratch.path().join("package");

    for (command, target) in [("glb", &out), ("package", &package)] {
        let mut run = p4kx_testkit::p4kx()?;
        run.env("P4KX_ARCHIVE_KEY", real_archive_key()?.to_hex());
        let output = run
            .args(["entity", command])
            .arg(real_archive_path()?)
            .arg(&pick.class)
            .arg("--out")
            .arg(target)
            .output()?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{command}: {stderr}");
        assert!(stderr.contains(&pick.geometry), "{command}: {stderr}");
        assert!(!target.exists(), "{command}: nothing is written");
    }
    println!(
        "{} ({}): exit code 2, file named",
        pick.class, pick.geometry
    );
    Ok(())
}
