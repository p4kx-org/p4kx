// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The item ports of the skins of the Entities whose geometry is a character file outside the
//! human rigs, picked by the census rule, anchor to the bones of the skeleton of the root.

#![expect(clippy::print_stdout, reason = "the counts are the output")]

use std::collections::HashSet;
use std::error::Error;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, ENTITY_TYPE, ForgePalettes, Geometry,
    IssueKind, LoadoutResolver, Placement,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_gltf::GlbOptions;
use p4kx_model::Model;
use p4kx_testkit::{check_glb, real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

/// Returns the lower-cased bone names of the root model of `assembly`.
fn skeleton_bones(assembly: &Assembly) -> HashSet<String> {
    assembly
        .root()
        .and_then(|root| root.model())
        .and_then(|model| Some(model.bones_of(model.lods.first()?)))
        .map_or_default(|bones| {
            bones
                .iter()
                .map(|bone| bone.name.to_ascii_lowercase())
                .collect()
        })
}

/// The Kopion variants of LIVE that wear a collar.
const MIN_COLLARS: usize = 5;

/// The helper of the tongue port of the Quasigrazer, which no skeleton holds.
const TONGUE_HELPER: &str = "Tongue_Attach_Point";

/// What the census rule found in the Assemblies of the creatures.
#[derive(Default)]
struct Found {
    creatures: usize,
    /// A line per item port of a skin the skin's own model did not resolve and the skeleton did.
    anchored: Vec<String>,
    /// Whether a creature lists the tongue helper as not found.
    tongue_listed: bool,
}

/// Returns whether `model` has a node or a bone named `name`, ignoring ASCII case.
fn holds(model: &Model, name: &str) -> bool {
    let bones = model
        .lods
        .first()
        .map_or(model.bones.as_slice(), |lod| model.bones_of(lod));
    model
        .nodes
        .iter()
        .map(|node| node.name.as_str())
        .chain(bones.iter().map(|bone| bone.name.as_str()))
        .any(|held| held.eq_ignore_ascii_case(name))
}

/// Asserts that no helper the skeleton holds is listed as missing, and records the item ports of
/// skins that are anchored to a bone of the skeleton by the fallback.
fn check(assembly: &Assembly, class: &str, found: &mut Found) {
    let bones = skeleton_bones(assembly);
    for issue in assembly.issues() {
        if let IssueKind::HelperNotFound { helper } = issue.kind() {
            assert!(
                !bones.contains(&helper.to_ascii_lowercase()),
                "{class}: {} lists {helper}, a bone of its skeleton",
                issue.path()
            );
            found.tongue_listed |= helper.eq_ignore_ascii_case(TONGUE_HELPER);
        }
    }
    let parts = assembly.parts();
    for part in parts {
        let Some(parent) = part.parent().and_then(|parent| parts.get(parent)) else {
            continue;
        };
        if let (Placement::Skin, Placement::Helper(helper)) = (parent.placement(), part.placement())
            && bones.contains(&helper.to_ascii_lowercase())
            && !parent.model().is_some_and(|skin| holds(skin, helper))
        {
            found
                .anchored
                .push(format!("{class}: {} at {helper}", part.path()));
        }
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_helper_the_creature_skeleton_holds_is_never_listed_as_missing() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let models = ArchiveModels::new(&archive);
    let palettes = ForgePalettes::new(&forge);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_files(&files)
        .stopping_at_docked_vehicles();
    let mut found = Found::default();
    for record in forge
        .records()
        .filter(|record| record.type_name().is_ok_and(|name| name == ENTITY_TYPE))
    {
        let Ok(tree) = resolver.resolve(&record) else {
            continue;
        };
        if !tree
            .geometry()
            .is_some_and(Geometry::is_rigged_file_outside_human_rigs)
        {
            continue;
        }
        let assembly =
            Assembly::build_with_palettes(&tree, &models, &palettes, &AssemblyOptions::default())?;
        let mut bytes = Vec::new();
        assembly.write_glb(&GlbOptions::default(), &mut bytes)?;
        check_glb(&bytes).map_err(|error| format!("{}: {error}", tree.class_name()))?;
        found.creatures += 1;
        check(&assembly, tree.class_name(), &mut found);
    }
    println!(
        "{} creature Entities, {} item ports of skins anchored to a bone",
        found.creatures,
        found.anchored.len()
    );
    for line in &found.anchored {
        println!("    {line}");
    }
    assert!(
        found.anchored.len() >= MIN_COLLARS,
        "the Kopion collars are anchored: {:?}",
        found.anchored
    );
    for line in &found.anchored {
        assert!(
            line.to_ascii_lowercase().starts_with("kopion_") && line.contains("/Collar_ItemPort"),
            "{line}: not a Kopion collar"
        );
    }
    assert!(found.tongue_listed, "the tongue helper stays a listed note");
    Ok(())
}
