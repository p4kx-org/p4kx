// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The teeth and eyes of generic NPCs in a real Archive. The Records `Head_Teeth` and
//! `Head_Eyes_*` name a skin per `Protos_Head` and body (`Male` / `Female`); the flag of the
//! head's ports covers only the first tag, so the body of the NPC decides, in the Entity tree and
//! in the Armour alike.

use std::error::Error;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Armour, Assembly, AssemblyOptions, LoadoutResolver,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const MALE: &str = "NPC_Archetypes-Male-Human-Civilians-Orison-Hauler_Orison_02";
const FEMALE: &str = "NPC_Archetypes-Female-Human-ShopKeeper-NewBabbage_Bureaucrat_Nest_01";
const MALE_FOLDER: &str = "heads/male/pu/protos_human_male_face_t1_pu/";
const FEMALE_FOLDER: &str = "heads/female/pu/protos_human_female_face_t1_pu/";
const PARTS: [(&str, &str); 2] = [
    ("Teeth_ItemPort", "_teeth.skin"),
    ("Eyes_ItemPort", "_eyes.skin"),
];

fn unified(path: &str) -> String {
    path.to_ascii_lowercase().replace('\\', "/")
}

fn assert_in_folder(files: &[(String, String)], folder: &str, other: &str) {
    for (name, suffix) in PARTS {
        let file = files
            .iter()
            .find(|(part, _)| part == name)
            .map(|(_, file)| file.as_str());
        assert!(
            file.is_some_and(|file| file.contains(folder)
                && file.ends_with(suffix)
                && !file.contains(other)),
            "{name}: {file:?} in {files:?}"
        );
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_teeth_and_eyes_of_generic_npcs_follow_their_body_in_the_armour_and_the_entity_assembly()
-> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let models = ArchiveModels::new(&archive);
    let options = AssemblyOptions::default().with_lod(0);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    for (class, folder, other) in [
        (MALE, MALE_FOLDER, FEMALE_FOLDER),
        (FEMALE, FEMALE_FOLDER, MALE_FOLDER),
    ] {
        let record = resolver.entities().find(class)?;
        let tree = resolver.resolve(&record)?;

        let head = resolver.npc_head(&tree).ok_or(class)?;
        let armour = Armour::build(head.variant(), head.pieces(), &models, &options)?;
        let worn: Vec<(String, String)> = armour
            .parts()
            .iter()
            .map(|part| (part.name().to_owned(), unified(part.file())))
            .collect();
        assert_in_folder(&worn, folder, other);

        let assembly = Assembly::build(&tree, &models, &options)?;
        let drawn: Vec<(String, String)> = PARTS
            .iter()
            .filter_map(|(name, suffix)| {
                let file = assembly
                    .parts()
                    .iter()
                    .filter_map(|part| part.geometry())
                    .map(unified)
                    .find(|file| file.ends_with(suffix))?;
                Some(((*name).to_owned(), file))
            })
            .collect();
        assert_in_folder(&drawn, folder, other);
    }
    Ok(())
}
