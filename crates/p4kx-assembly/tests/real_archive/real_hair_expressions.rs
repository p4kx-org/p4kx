// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The hair, brows and lashes of the stock male head carry expression targets at the lower levels of
//! detail too, derived from the targets the head has there.

use std::collections::HashSet;
use std::error::Error;
use std::io::Read;

use p4kx_assembly::character::Chf;
use p4kx_assembly::{
    ArchiveModels, Armour, ArmourPart, AssemblyOptions, Expressions, ForgeItems, Head,
    LoadoutResolver,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_gltf::GlbOptions;
use p4kx_testkit::{check_glb, real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const HEAD: &str = "head";
/// The GLB with expressions stays under this many times the one without.
const MAX_GROWTH: usize = 5;
const FOLLOW_WEIGHTS_SKIPPED: &str = "follow_weights_skipped";
const FOLLOW_FIRST_MESH_ONLY: &str = "follow_morphs_first_mesh_only";

fn shipped(archive: &Archive, name: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path().to_ascii_lowercase().ends_with(name))
        .ok_or_else(|| format!("the Archive ships no {name}"))?;
    let mut bytes = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// The size of a GLB and the morph targets it writes.
struct Written {
    bytes: usize,
    targets: usize,
}

/// Writes the GLB of `armour` at `lod` and checks it.
fn written(armour: &Armour, lod: u32) -> Result<Written, Box<dyn Error>> {
    let mut bytes = Vec::new();
    let summary = armour.write_glb(&GlbOptions::default().with_lod(lod), &mut bytes)?;
    check_glb(&bytes)?;
    Ok(Written {
        bytes: bytes.len(),
        targets: summary.morph_targets,
    })
}

fn first_level_meshes(part: &ArmourPart) -> usize {
    part.model().lods.first().map_or(0, |lod| lod.meshes.len())
}

/// Returns the parts that follow the head and have targets: all but the head with morphs.
fn followers(armour: &Armour) -> impl Iterator<Item = &ArmourPart> {
    armour
        .parts()
        .iter()
        .filter(|part| part.name() != HEAD && !part.morphs().is_empty())
}

/// Checks the targets of `part` against the head's target names `head_names`.
fn check_follower(lod: u32, part: &ArmourPart, head_names: &HashSet<&str>) {
    let vertices = part
        .model()
        .lods
        .first()
        .and_then(|lod| lod.meshes.first())
        .map_or(0, |mesh| mesh.positions.len());
    assert!(part.morphs().len() <= head_names.len(), "LOD{lod}");
    for target in part.morphs() {
        let name = &target.name;
        assert!(head_names.contains(name.as_str()), "LOD{lod}: {name}");
        assert!(!target.vertices.is_empty(), "LOD{lod}: {name}");
        assert!(
            target
                .vertices
                .iter()
                .all(|&vertex| usize::try_from(vertex).is_ok_and(|at| at < vertices)),
            "LOD{lod}: {name}"
        );
        assert!(
            target
                .deltas
                .iter()
                .flatten()
                .all(|value| value.is_finite()),
            "LOD{lod}: {name}"
        );
    }
}

/// Checks the reports of `armour`: no part loses its animation weights, and the first-mesh note
/// appears exactly when a part that follows the head has several meshes.
fn check_issues(lod: u32, armour: &Armour) {
    let codes: Vec<&str> = armour
        .issues()
        .iter()
        .map(|issue| issue.kind().code())
        .collect();
    assert!(
        !codes.contains(&FOLLOW_WEIGHTS_SKIPPED),
        "LOD{lod}: {codes:?}"
    );
    let several = followers(armour).any(|part| first_level_meshes(part) > 1);
    assert_eq!(
        codes.contains(&FOLLOW_FIRST_MESH_ONLY),
        several,
        "LOD{lod}: {codes:?}"
    );
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn what_follows_the_stock_male_head_has_its_expressions_at_every_level() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let models = ArchiveModels::new(&archive);
    let chf = Chf::read(&shipped(&archive, "masculinedefault.chf")?)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let mut items = ForgeItems::new(&mut resolver);
    let mut build = |expressions: &Expressions, lod: u32| {
        let head = Head::dressed(&chf.character, &mut items)?.with_expressions(expressions);
        Armour::build(
            head.variant(),
            head.pieces(),
            &models,
            &AssemblyOptions::default().with_lod(lod),
        )
        .map_err(Box::<dyn Error>::from)
    };

    for lod in 0..=3 {
        let plain = build(&Expressions::None, lod)?;
        let armour = build(&Expressions::All, lod)?;
        check_targets(lod, &armour)?;
        check_issues(lod, &armour);
        check_glb_sizes(lod, &armour, &plain)?;
    }
    Ok(())
}

#[expect(clippy::print_stdout, reason = "the target counts are the measurement")]
fn check_targets(lod: u32, armour: &Armour) -> TestResult {
    let head = armour
        .parts()
        .iter()
        .find(|part| part.name() == HEAD)
        .ok_or("the head")?;
    let head_names: HashSet<&str> = head
        .morphs()
        .iter()
        .map(|target| target.name.as_str())
        .collect();
    assert!(
        followers(armour).next().is_some(),
        "LOD{lod}: nothing follows the head"
    );
    for part in followers(armour) {
        println!(
            "LOD{lod}: {} has {} targets of {} on the head",
            part.name(),
            part.morphs().len(),
            head_names.len()
        );
        check_follower(lod, part, &head_names);
    }
    Ok(())
}

#[expect(clippy::print_stdout, reason = "the sizes are the measurement")]
fn check_glb_sizes(lod: u32, armour: &Armour, plain: &Armour) -> TestResult {
    let targets: usize = armour.parts().iter().map(|part| part.morphs().len()).sum();
    let (with, without) = (written(armour, lod)?, written(plain, lod)?);
    println!(
        "LOD{lod}: GLB {} bytes without expressions, {} with",
        without.bytes, with.bytes
    );
    assert_eq!(
        with.targets, targets,
        "LOD{lod}: the GLB carries every target"
    );
    assert_eq!(without.targets, 0, "LOD{lod}");
    assert!(
        with.bytes < MAX_GROWTH * without.bytes,
        "LOD{lod}: {} bytes with, {} without",
        with.bytes,
        without.bytes
    );
    Ok(())
}
