// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The own head skins of a real story character in the Entity Assembly: the head, teeth and eyes
//! of Miles Eckhart are his own, in the folder of his head, as in the Armour of his Loadout.

use std::error::Error;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, LoadoutResolver, NpcHead,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const NPC: &str = "NPC_Archetypes-Male-Human-MissionGivers-MilesEckhart";
const PREFIX: &str = "eckhart_t2";

fn skins() -> Result<(Vec<String>, NpcHead), Box<dyn Error>> {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let models = ArchiveModels::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let record = resolver.entities().find(NPC)?;
    let tree = resolver.resolve(&record)?;
    let head = resolver.npc_head(&tree).ok_or("the NPC has no head")?;
    let options = AssemblyOptions::default().with_lod(0);
    let assembly = Assembly::build(&tree, &models, &options)?;
    let files = assembly
        .parts()
        .iter()
        .filter_map(|part| part.geometry().map(str::to_owned))
        .collect();
    Ok((files, head))
}

fn own(files: &[String], part: &str) -> bool {
    let name = format!("{PREFIX}_{part}.skin");
    files
        .iter()
        .any(|file| file.to_ascii_lowercase().ends_with(&name))
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_head_teeth_and_eyes_of_miles_eckhart_are_his_own_in_the_entity_assembly() -> TestResult {
    let (files, head) = skins()?;
    let generic: Vec<&String> = files
        .iter()
        .filter(|file| {
            let file = file.to_ascii_lowercase();
            ["_head", "_teeth", "_eyes"]
                .iter()
                .any(|part| file.contains("protos_human_male_face_t1_pu") && file.contains(part))
        })
        .collect();
    assert!(generic.is_empty(), "{generic:?}");
    for part in ["head", "teeth", "eyes"] {
        assert!(own(&files, part), "{part} in {files:?}");
    }
    let folder = head.folder().ok_or("the head has no folder of its own")?;
    for file in files.iter().filter(|file| file.contains(PREFIX)) {
        assert!(
            file.to_ascii_lowercase()
                .starts_with(&folder.to_ascii_lowercase()),
            "{file} outside {folder}"
        );
    }
    Ok(())
}
