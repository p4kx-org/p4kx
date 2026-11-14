// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_assembly::{ArchiveModels, Armour, AssemblyOptions, BodyVariant, PartOrigin};
use p4kx_extraction::model::ModelFile;
use p4kx_extraction::{Archive, OpenOptions, SourceEncryption};
use p4kx_testkit::{ArchiveBuilder, armour_entries, test_key};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn body_skeleton_and_only_the_selected_skin_retain_their_archive_inputs() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("armour.p4k");
    let mut builder = ArchiveBuilder::new();
    for entry in armour_entries() {
        let name = entry.path();
        let entry = if name.ends_with("m_body.skinm") || name.ends_with("f_body.skinm") {
            entry.encrypted()
        } else {
            entry
        };
        builder = builder.entry(entry);
    }
    builder.write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let armour = Armour::build(
        BodyVariant::Male,
        &[],
        &ArchiveModels::new(&archive),
        &AssemblyOptions::default(),
    )?;
    let skeleton = armour
        .skeleton_archive_source()
        .ok_or("body skeleton source")?;
    let skeleton_entry = skeleton.origin().ok_or("body skeleton Entry")?;
    assert_eq!(skeleton.file(), ModelFile::Model);
    assert!(
        skeleton_entry
            .entry()
            .path()
            .ends_with("bhm_skeleton_v7.chr")
    );
    assert_eq!(skeleton_entry.encryption(), SourceEncryption::Unencrypted);
    let [part] = armour.parts() else {
        return Err("the only worn part must be the default body".into());
    };
    assert_eq!(part.origin(), PartOrigin::Body);
    assert!(part.model_definition_origin().is_none());
    let sources: Vec<_> = part
        .model_archive_sources()
        .iter()
        .filter_map(|source| {
            source
                .origin()
                .map(|origin| (source.file(), origin.entry().path(), origin.encryption()))
        })
        .collect();
    let [
        (ModelFile::Model, model, SourceEncryption::Unencrypted),
        (ModelFile::Geometry, geometry, SourceEncryption::Encrypted),
    ] = sources.as_slice()
    else {
        return Err(format!("unexpected skin inputs: {sources:?}").into());
    };
    assert!(model.ends_with("m_body.skin"));
    assert!(geometry.ends_with("m_body.skinm"));
    Ok(())
}
