// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The clothing of a generic NPC in a real Archive: its head names no body, so the body entity
//! decides, and no worn item is a world prop such as a cardboard box.

use std::error::Error;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, Attachment, BodyVariant, EntityNode,
    IssueKind, LoadoutResolver,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const MALE: &str = "NPC_Archetypes-Male-Human-Civilians-Orison-Hauler_Orison_02";
const FEMALE: &str = "NPC_Archetypes-Female-Human-ShopKeeper-NewBabbage_Bureaucrat_Nest_01";
const FLUID_PRISONER: &str = "NPC_Archetypes_Male_Human_Civilians_Prisoner_13";
const LEGS_PORT: &str = "Body_ItemPort/Clothing_Legs";
const UNDERSUIT_PORT: &str = "Body_ItemPort/Armor_Undersuit";
const NPC_MARKER: &str = "NPC_Archetypes";
const ENTITY_TYPE: &str = "EntityClassDefinition";
const CHARACTER_PREFIX: &str = "char_";
const CHARACTERS_FOLDER: &str = "objects/characters/";
const CHARACTER_EXTENSIONS: [&str; 3] = [".cdf", ".chr", ".skin"];

/// Every `Char_` Entity of the Archive without sub-geometry per body whose main geometry is a
/// world prop: a placeholder jacket and a placeholder shirt, nothing to wear (ADR-0311).
const WORLD_PROP_ONLY_ITEMS: [&str; 2] = [
    "Clothing_Torso2_Unified [Char_Clothing_Torso_2] Objects/buildingsets/human/universal/prop/storage/box_cardboard/box_clothing_1_jacket_a.cgf",
    "bnu_shirt_01_01_01 [Char_Clothing_Torso_0] Objects/buildingsets/human/universal/prop/storage/box_cardboard/box_clothing_1_shirt_a.cgf",
];

/// Returns `path` in lower case with `/`.
fn unified(path: &str) -> String {
    path.to_ascii_lowercase().replace('\\', "/")
}

/// Returns whether the model `path` is a prop of the world: neither in the characters folder nor
/// a character file.
fn is_world_prop(path: &str) -> bool {
    let path = unified(path);
    !path.starts_with(CHARACTERS_FOLDER)
        && !CHARACTER_EXTENSIONS
            .iter()
            .any(|extension| path.ends_with(extension))
}

/// Returns the geometry of the character items below `tree`, by port path.
fn worn_geometry(tree: &EntityNode) -> Vec<(String, Option<String>)> {
    tree.walk()
        .into_iter()
        .filter_map(|(path, port)| match port.attachment() {
            Attachment::Entity(node)
                if node
                    .attach_type()
                    .to_ascii_lowercase()
                    .starts_with(CHARACTER_PREFIX) =>
            {
                Some((
                    path,
                    node.geometry().map(|geometry| unified(geometry.path())),
                ))
            }
            _ => None,
        })
        .collect()
}

fn props(tree: &EntityNode) -> Vec<(String, String)> {
    worn_geometry(tree)
        .into_iter()
        .filter_map(|(path, geometry)| Some((path, geometry?)))
        .filter(|(_, geometry)| is_world_prop(geometry))
        .collect()
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn generic_npcs_wear_the_skins_of_their_body_and_no_world_prop() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    for (class, folder, port) in [
        (MALE, "male_v7", LEGS_PORT),
        (FEMALE, "female_v2", LEGS_PORT),
        (FLUID_PRISONER, "male_v7", UNDERSUIT_PORT),
    ] {
        let record = resolver.entities().find(class)?;
        let tree = resolver.resolve(&record)?;
        let worn = worn_geometry(&tree)
            .into_iter()
            .find(|(path, _)| path == port)
            .and_then(|(_, geometry)| geometry)
            .ok_or_else(|| format!("{class}: nothing at {port}"))?;
        assert!(worn.contains(folder), "{class}: {worn}");
        assert!(props(&tree).is_empty(), "{class}: {:?}", props(&tree));
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn no_story_or_generic_npc_wears_a_world_prop() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let records: Vec<_> = forge
        .records()
        .filter(|record| record.type_name().is_ok_and(|name| name == ENTITY_TYPE))
        .filter(|record| record.name().is_ok_and(|name| name.contains(NPC_MARKER)))
        .collect();
    assert!(records.len() > 1000, "{} NPC records", records.len());
    let (mut offenders, mut unresolved) = (Vec::new(), 0_usize);
    for record in &records {
        let Ok(tree) = resolver.resolve(record) else {
            unresolved += 1;
            continue;
        };
        for (path, geometry) in props(&tree) {
            offenders.push(format!(
                "{} {path} {geometry}",
                record.name().unwrap_or_default()
            ));
        }
    }
    assert_eq!(
        unresolved,
        0,
        "of {} NPC records do not resolve",
        records.len()
    );
    assert!(
        offenders.is_empty(),
        "{} world props, e.g. {:?}",
        offenders.len(),
        &offenders[..offenders.len().min(5)]
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_character_items_that_name_only_a_world_prop_are_the_census_of_the_adr() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let records: Vec<_> = forge
        .records()
        .filter(|record| record.type_name().is_ok_and(|name| name == ENTITY_TYPE))
        .collect();
    let (mut found, mut unreadable) = (Vec::new(), 0_usize);
    for record in &records {
        let Ok(wearable) = resolver.wearable(record) else {
            unreadable += 1;
            continue;
        };
        if !wearable
            .item_type()
            .to_ascii_lowercase()
            .starts_with(CHARACTER_PREFIX)
        {
            continue;
        }
        let per_body = wearable.variants().iter().any(|variant| {
            BodyVariant::ALL
                .iter()
                .any(|body| variant.has_tag(body.tag()))
        });
        if let Some(main) = wearable.geometry()
            && !per_body
            && is_world_prop(main.path())
        {
            found.push(format!(
                "{} [{}] {}",
                wearable.class_name(),
                wearable.item_type(),
                main.path()
            ));
        }
    }
    found.sort_unstable();
    assert_eq!(unreadable, 0, "unreadable Entity Records");
    assert_eq!(found, WORLD_PROP_ONLY_ITEMS);
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_character_body_and_the_npc_head_of_a_generic_npc_follow_its_body_entity() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    for (class, variant) in [(MALE, BodyVariant::Male), (FEMALE, BodyVariant::Female)] {
        let record = resolver.entities().find(class)?;
        let tree = resolver.resolve(&record)?;
        let body = tree.character_body().ok_or(class)?;
        assert_eq!(body.variant(), variant, "{class}");
        assert!(!body.is_named_by_head(), "{class}");
        let head = resolver.npc_head(&tree).ok_or(class)?;
        assert_eq!(head.variant(), variant, "{class}");
        assert_eq!(head.class_name(), body.head().class_name(), "{class}");
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn most_npc_archetypes_have_a_character_body_named_by_their_body_entity() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let (mut by_head, mut by_body, mut none) = (0_usize, 0_usize, 0_usize);
    for record in forge
        .records()
        .filter(|record| record.type_name().is_ok_and(|name| name == ENTITY_TYPE))
        .filter(|record| record.name().is_ok_and(|name| name.contains(NPC_MARKER)))
    {
        let Ok(tree) = resolver.resolve(&record) else {
            continue;
        };
        match tree.character_body() {
            Some(body) if body.is_named_by_head() => by_head += 1,
            Some(_) => by_body += 1,
            None => none += 1,
        }
    }
    assert!(by_body > 1000, "{by_body} by body, {none} none");
    assert!(
        by_body > by_head,
        "{by_head} named by head, {by_body} by body"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_body_of_a_generic_npc_is_cut_under_its_clothing() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let models = ArchiveModels::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let record = resolver.entities().find(MALE)?;
    let tree = resolver.resolve(&record)?;

    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default().with_lod(0))?;

    let hidden: Vec<_> = assembly
        .issues()
        .iter()
        .filter_map(|issue| match issue.kind() {
            IssueKind::BodyZonesHidden(zones) => Some(zones.len()),
            _ => None,
        })
        .collect();
    assert_eq!(hidden.len(), 1, "{:?}", assembly.issues());
    assert!(hidden[0] > 0);
    Ok(())
}
