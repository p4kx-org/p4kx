// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestModel, entity_schema,
    write_build_manifest,
};

use super::{Selection, TestResult, verify};

const MODEL: &str = "Data/Objects/witness.cdf";
const ENTITY: &str = "FixtureScope";

#[test]
fn synthetic_animation_scope_witness_checks_model_and_entity_publications() -> TestResult {
    let directory = tempfile::tempdir()?;
    write_build_manifest(directory.path(), "fixture", "1.2.3", "42", "scope-fixture")?;
    let archive = archive(directory.path())?;
    let build = archive.build().ok_or("fixture Build")?;
    let watch = ArchiveWatch::new(&archive);
    for selection in [
        Selection::Model(MODEL.to_owned()),
        Selection::Entity(ENTITY.to_owned()),
    ] {
        let expected = selection.discover(&archive, &watch)?;
        assert!(
            expected.has_events(),
            "the synthetic scope contains an authored event database"
        );
        verify(&archive, build, &selection, expected)?;
    }
    watch.check()?;
    Ok(())
}

fn archive(root: &Path) -> TestResult<Archive> {
    let entity = TestEntity::new(1, ENTITY, "entities/fixture_scope.xml")
        .geometry("Objects/witness.cdf", None);
    let data = entity_schema(DataForgeBuilder::new())
        .record(entity.record())
        .build();
    let path = root.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", data))
        .entry(TestEntry::stored(
            MODEL,
            br#"<CharacterDefinition><Model File="Objects/witness.chr"/></CharacterDefinition>"#,
        ))
        .entry(TestEntry::stored(
            "Data/Objects/witness.chr",
            TestModel::cube().cgf(),
        ))
        .entry(TestEntry::stored(
            "Data/Objects/witness.chrparams",
            br#"<Params><AnimationList>
                <Animation name="clip" path="Animations/clip.caf"/>
                <Animation name="$AnimEventDatabase" path="Animations/witness.animevents"/>
                </AnimationList></Params>"#,
        ))
        .entry(TestEntry::stored(
            "Data/Animations/witness.animevents",
            br#"<anim_event_list><animation name="Animations/clip.caf">
                <event name="cue" time="0.5"/></animation></anim_event_list>"#,
        ))
        .entry(TestEntry::stored(
            "Data/Animations/clip.caf",
            b"reference target",
        ))
        .entry(TestEntry::stored(
            "Data/Objects/unrelated.socpak",
            b"excluded malformed container",
        ))
        .write_to(&path)?;
    Ok(Archive::open(path)?)
}
