// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The entity path draws the own skins of a story character's head, as the armour path does: the
//! head and its parts take `<prefix>_<part>.skin` from the folder of the head's material library
//! when the Archive holds it.

use std::error::Error;

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, IssueKind, LoadoutResolver};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{NPC_CLASS, NPC_HEAD_FOLDER, NPC_HEAD_MATERIAL, TestEntry, npc_entries};

use crate::assembly_support::{TestResult, fixture};

fn assemble(entries: Vec<TestEntry>) -> Result<Assembly, Box<dyn Error>> {
    let fixture = fixture(entries)?;
    let archive = Archive::open(&fixture.path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find(NPC_CLASS)?;
    let tree = resolver.resolve(&record)?;
    let models = ArchiveModels::new(&archive);
    Ok(Assembly::build(
        &tree,
        &models,
        &AssemblyOptions::default(),
    )?)
}

fn files(assembly: &Assembly) -> Vec<&str> {
    assembly
        .parts()
        .iter()
        .filter_map(|part| part.geometry())
        .collect()
}

fn has(files: &[&str], own: &str) -> bool {
    files.iter().any(|file| file.eq_ignore_ascii_case(own))
}

#[test]
fn the_head_and_its_parts_take_the_skins_of_the_characters_own_folder() -> TestResult {
    let assembly = assemble(npc_entries())?;
    let files = files(&assembly);
    for part in ["head", "eyes"] {
        let own = format!("{NPC_HEAD_FOLDER}/npc_test_t1_{part}.skin");
        assert!(has(&files, &own), "{own} in {files:?}");
    }
    assert!(
        !files
            .iter()
            .any(|file| file.to_ascii_lowercase().contains("protos_human_male_face")),
        "{files:?}"
    );
    Ok(())
}

#[test]
fn a_skin_the_archive_lacks_leaves_the_generic_one_without_a_note() -> TestResult {
    let entries: Vec<TestEntry> = npc_entries()
        .into_iter()
        .filter(|entry| {
            !entry
                .path()
                .to_ascii_lowercase()
                .contains("npc_test_t1_eyes.skin")
        })
        .collect();
    let assembly = assemble(entries)?;
    let files = files(&assembly);
    assert!(
        files
            .iter()
            .any(|file| file.ends_with("protos_human_male_face_t1_pu_eyes.skin")),
        "{files:?}"
    );
    assert!(has(
        &files,
        &format!("{NPC_HEAD_FOLDER}/npc_test_t1_head.skin")
    ));
    let codes: Vec<&str> = assembly
        .issues()
        .iter()
        .map(|issue| issue.kind().code())
        .collect();
    assert!(!codes.contains(&"sub_geometry_missing"), "{codes:?}");
    assert!(!codes.contains(&"geometry_missing"), "{codes:?}");
    Ok(())
}

#[test]
fn a_part_whose_library_is_missing_takes_the_library_of_the_head_and_says_so() -> TestResult {
    let entries: Vec<TestEntry> = npc_entries()
        .into_iter()
        .filter(|entry| {
            !entry
                .path()
                .to_ascii_lowercase()
                .contains("npc_test_t1_eyes_material.mtl")
        })
        .collect();
    let assembly = assemble(entries)?;
    let eyes = assembly
        .parts()
        .iter()
        .find(|part| part.name().starts_with("Eyes_ItemPort/"))
        .ok_or("the eyes part")?;
    let library = eyes
        .materials()
        .library()
        .ok_or("the eyes have no library")?;
    assert!(
        library.path.eq_ignore_ascii_case(NPC_HEAD_MATERIAL),
        "{}",
        library.path
    );
    let notes: Vec<_> = assembly
        .issues()
        .iter()
        .filter(|issue| matches!(issue.kind(), IssueKind::MaterialLibrary { .. }))
        .collect();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(!notes[0].kind().skips());
    Ok(())
}
