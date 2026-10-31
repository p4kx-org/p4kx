// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Entity Assembly of a real NPC in layered garments: an undersuit hides the shirt, the pants
//! and the boots under it, and the armour over it stays.

use std::error::Error;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, IssueKind, LoadoutResolver,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const WEARER: &str = "NPC_Archetypes-Male-Human-MissionGivers-Forge";
const SUIT: &str = "Armor_Undersuit/srvl_undersuit_02_01_02";

fn assemble() -> Result<Assembly, Box<dyn Error>> {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let models = ArchiveModels::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let record = resolver.entities().find(WEARER)?;
    let tree = resolver.resolve(&record)?;
    let options = AssemblyOptions::default().with_lod(0);
    Ok(Assembly::build(&tree, &models, &options)?)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn an_undersuit_hides_the_garments_under_it_in_the_entity_assembly() -> TestResult {
    let assembly = assemble()?;

    let mut hidden: Vec<(&str, &str)> = assembly
        .issues()
        .iter()
        .filter_map(|issue| match issue.kind() {
            IssueKind::PartHidden { port, by } => Some((port.as_str(), by.as_str())),
            _ => None,
        })
        .collect();
    hidden.sort_unstable();
    assert_eq!(
        hidden,
        [
            ("Clothing_Feet", SUIT),
            ("Clothing_Legs", SUIT),
            ("Clothing_Torso_0", SUIT),
        ]
    );
    for part in assembly.parts() {
        let left_out = ["Clothing_Feet/", "Clothing_Legs/", "Clothing_Torso_0/"]
            .iter()
            .any(|port| part.name().starts_with(port));
        assert_eq!(part.is_hidden(), left_out, "{}", part.name());
        assert!(!left_out || part.model().is_none(), "{}", part.name());
    }
    Ok(())
}
