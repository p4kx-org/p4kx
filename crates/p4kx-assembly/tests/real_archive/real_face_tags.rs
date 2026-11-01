// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The sub-geometry the eyelash and eyebrow ports of story characters take in a real Archive: the
//! entity path and the armour path agree.

use std::error::Error;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Armour, AssemblyOptions, Attachment, EntityNode, Geometry,
    LoadoutResolver,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};

use crate::real_paths::unified;

type TestResult = Result<(), Box<dyn Error>>;
/// A port name with the file of the entity path and the file of the armour path.
type PortFiles = (String, Option<String>, Option<String>);

const ECKHART: &str = "NPC_Archetypes-Male-Human-MissionGivers-MilesEckhart";
const PACHECO: &str = "NPC_Archetypes-Female-Human-MissionGivers-TeciaPacheco";
const LASHES: &str = "Eyelashes_ItemPort";
const BROW: &str = "Eyebrow_ItemPort";
const PORTS: [&str; 2] = [LASHES, BROW];

/// Returns the entity-path file of the port `name` among `files`.
fn file_of<'f>(files: &'f [PortFiles], name: &str) -> Result<&'f str, Box<dyn Error>> {
    files
        .iter()
        .find(|(port, _, _)| port == name)
        .and_then(|(_, entity, _)| entity.as_deref())
        .ok_or_else(|| format!("no file at {name}").into())
}

/// Returns the file the Entity on the head's port `name` has as its geometry.
fn entity_file(head: &EntityNode, name: &str) -> Option<String> {
    head.ports()
        .iter()
        .filter(|port| port.name() == name)
        .find_map(|port| match port.attachment() {
            Attachment::Entity(item) => item
                .geometry()
                .map(|geometry| unified(Geometry::path(geometry))),
            _ => None,
        })
}

/// Returns, per port of [`PORTS`], the file of the entity path and the file of the armour path.
fn files_of(class: &str) -> Result<Vec<PortFiles>, Box<dyn Error>> {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let models = ArchiveModels::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let record = resolver.entities().find(class)?;
    let tree = resolver.resolve(&record)?;
    let head = resolver.npc_head(&tree).ok_or("no head")?;
    let options = AssemblyOptions::default().with_lod(0);
    let armour = Armour::build(head.variant(), head.pieces(), &models, &options)?;
    let body = tree.character_body().ok_or("no body")?;
    Ok(PORTS
        .iter()
        .map(|name| {
            let worn = armour
                .parts()
                .iter()
                .find(|part| part.name() == *name)
                .map(|part| unified(part.file()));
            ((*name).to_owned(), entity_file(body.head(), name), worn)
        })
        .collect())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_eyelashes_of_miles_eckhart_are_his_own_on_both_paths() -> TestResult {
    let files = files_of(ECKHART)?;
    for (port, entity, armour) in &files {
        assert_eq!(entity, armour, "{port}");
    }
    let lashes = file_of(&files, LASHES)?;
    assert!(lashes.ends_with("/eckhart_t2_eyelashes.skin"), "{lashes}");
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_face_parts_of_tecia_pacheco_are_the_same_on_both_paths() -> TestResult {
    let files = files_of(PACHECO)?;
    for (port, entity, armour) in &files {
        assert_eq!(entity, armour, "{port}");
    }
    let brow = file_of(&files, BROW)?;
    assert!(brow.ends_with("/pacheco_t3_eyelashes.skin"), "{brow}");
    let lashes = file_of(&files, LASHES)?;
    assert!(
        lashes.ends_with("/protos_human_female_face_t1_pu_eyedetails.skin"),
        "{lashes}"
    );
    Ok(())
}
