// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The eyelashes of a female generic NPC in a real Archive. The Record `Head_Eyelashes` gives its
//! `Female` sub-geometry `f_lashes_01.skin` the library `lashes_01_01.mtl` of the shared male
//! folder; that skin names the same library itself, with the two hair-card sub-materials its
//! triangles use. The only female lash library in the Archive, the `eyelashes_material.mtl` of the
//! female protos head folder, belongs to no skin and no Record names it, so the output follows the
//! Record.

use std::error::Error;

use p4kx_assembly::{ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, LoadoutResolver};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_model::MaterialStatus;
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const FEMALE: &str = "NPC_Archetypes-Female-Human-ShopKeeper-NewBabbage_Bureaucrat_Nest_01";
const FEMALE_LASHES_SKIN: &str = "heads/shared/lashes/female/shared/f_lashes_01.skin";
const SHARED_LASHES_LIBRARY: &str = "heads/shared/lashes/male/shared/lashes_01_01.mtl";

fn unified(path: &str) -> String {
    path.to_ascii_lowercase().replace('\\', "/")
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_lashes_of_a_female_generic_npc_use_the_library_of_their_record_and_skin() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let models = ArchiveModels::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let record = resolver.entities().find(FEMALE)?;
    let tree = resolver.resolve(&record)?;
    let options = AssemblyOptions::default().with_lod(0);
    let assembly = Assembly::build(&tree, &models, &options)?;
    let lashes = assembly
        .parts()
        .iter()
        .find(|part| {
            part.geometry()
                .is_some_and(|path| unified(path).ends_with(FEMALE_LASHES_SKIN))
        })
        .ok_or("the NPC has no female lash part")?;
    let MaterialStatus::Loaded(library) = lashes.materials() else {
        return Err(format!("the lashes have no library: {:?}", lashes.materials()).into());
    };
    assert!(
        unified(&library.path).ends_with(SHARED_LASHES_LIBRARY),
        "{}",
        library.path
    );
    assert_eq!(
        library.materials.len(),
        2,
        "the two hair-card sub-materials"
    );
    Ok(())
}
