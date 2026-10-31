// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The props on the item ports of the skin of a human character hang on the bones their ports
//! name in the skeleton of the root: the mobiGlas on a skin port and the tumbler on a bone port.

#![expect(clippy::print_stdout, reason = "the counts are the output")]

use std::collections::{BTreeSet, HashSet};
use std::error::Error;
use std::path::Path;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, ENTITY_TYPE, EntityNode, ForgePalettes,
    Geometry, IssueKind, LoadoutResolver, Placement,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_gltf::GlbOptions;
use p4kx_testkit::{check_glb, real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const NPC: &str = "NPC_Archetypes-Male-Human-MissionGivers-MilesEckhart";
const MOBIGLAS_PORT: &str = "Body_ItemPort/mobiglas_attach";
const TUMBLER_PORT: &str = "Body_ItemPort/weapon_attach_hand_right";

/// The folder of the human body rigs, whose skeleton file a human root names.
const HUMAN_RIG: &str = "objects/characters/human/";

/// The fewest Entities with a human skeleton file a census of LIVE finds.
const MIN_HUMANS: usize = 100;

fn skeleton_bones(assembly: &Assembly) -> HashSet<String> {
    assembly
        .root()
        .and_then(|root| root.model())
        .and_then(|model| Some(model.bones_of(model.lods.first()?)))
        .map(|bones| {
            bones
                .iter()
                .map(|bone| bone.name.to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default()
}

/// Returns whether the root of `tree` names a skeleton file of the human body rigs.
fn has_human_skeleton(tree: &EntityNode) -> bool {
    tree.geometry().map(Geometry::path).is_some_and(|path| {
        let path = path.replace('\\', "/").to_ascii_lowercase();
        path.starts_with(HUMAN_RIG)
            && Path::new(&path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("chr"))
    })
}

/// Asserts that no helper the skeleton of the root holds is listed as missing.
fn check_helpers(assembly: &Assembly, class: &str) {
    let bones = skeleton_bones(assembly);
    for issue in assembly.issues() {
        if let IssueKind::HelperNotFound { helper } = issue.kind() {
            assert!(
                !bones.contains(&helper.to_ascii_lowercase()),
                "{class}: {} lists {helper}, a bone of its skeleton",
                issue.path()
            );
        }
    }
}

/// Returns a line per part that hangs on a bone below a skin part.
fn props_on_skins(assembly: &Assembly) -> Vec<String> {
    let parts = assembly.parts();
    parts
        .iter()
        .filter_map(|part| {
            let parent = parts.get(part.parent()?)?;
            let Placement::Helper(helper) = part.placement() else {
                return None;
            };
            (parent.placement() == &Placement::Skin)
                .then(|| format!("{} at {helper}", part.class_name()))
        })
        .collect()
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_mobiglas_and_the_tumbler_of_a_story_character_hang_on_their_bones() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let models = ArchiveModels::new(&archive);
    let palettes = ForgePalettes::new(&forge);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let record = resolver.entities().find(NPC)?;
    let tree = resolver.resolve(&record)?;
    let assembly =
        Assembly::build_with_palettes(&tree, &models, &palettes, &AssemblyOptions::default())?;

    let mobiglas = assembly.part_at(MOBIGLAS_PORT).ok_or("the mobiGlas")?;
    assert_eq!(
        mobiglas.placement(),
        &Placement::Helper("mobiglas_attach".to_owned())
    );
    let tumbler = assembly.part_at(TUMBLER_PORT).ok_or("the tumbler")?;
    assert_eq!(
        tumbler.placement(),
        &Placement::Helper("RightWeaponBone".to_owned())
    );
    for part in [mobiglas, tumbler] {
        assert_ne!(part.world().translation(), [0.0; 3], "{}", part.path());
    }
    assert!(
        assembly
            .issues()
            .iter()
            .all(|issue| !matches!(issue.kind(), IssueKind::HelperNotFound { .. })),
        "{:?}",
        assembly.issues()
    );
    let mut bytes = Vec::new();
    assembly.write_glb(&GlbOptions::default(), &mut bytes)?;
    check_glb(&bytes)?;
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn no_helper_the_skeleton_of_a_human_root_holds_is_listed_as_missing() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let models = ArchiveModels::new(&archive);
    let palettes = ForgePalettes::new(&forge);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_files(&files)
        .stopping_at_docked_vehicles();
    let mut humans = 0_usize;
    let mut anchored = BTreeSet::new();
    for record in forge
        .records()
        .filter(|record| record.type_name().is_ok_and(|name| name == ENTITY_TYPE))
    {
        let Ok(tree) = resolver.resolve(&record) else {
            continue;
        };
        if !has_human_skeleton(&tree) {
            continue;
        }
        let assembly =
            Assembly::build_with_palettes(&tree, &models, &palettes, &AssemblyOptions::default())?;
        let mut bytes = Vec::new();
        assembly.write_glb(&GlbOptions::default(), &mut bytes)?;
        check_glb(&bytes).map_err(|error| format!("{}: {error}", tree.class_name()))?;
        humans += 1;
        check_helpers(&assembly, tree.class_name());
        anchored.extend(props_on_skins(&assembly));
    }
    println!(
        "{humans} human Entities, {} kinds of prop on a skin's item port",
        anchored.len()
    );
    for line in &anchored {
        println!("    {line}");
    }
    assert!(humans >= MIN_HUMANS, "{humans} human Entities");
    assert!(
        anchored.iter().any(|line| line.contains("RightWeaponBone")),
        "a prop on the bone RightWeaponBone"
    );
    Ok(())
}
