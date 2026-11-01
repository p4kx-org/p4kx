// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The own skin of a story character's head gives way to what the Entity had without it, with the
//! reports of the Assembly as before: the sub-geometry the port flags chose, then the main
//! geometry. A head that has no own skins changes nothing.

use std::error::Error;

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, LoadoutResolver};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry, TestModel, TestPort, entity_schema};

use crate::assembly_support::{TestResult, fixture};

const LIBRARY: &str = "Objects/Characters/Human/heads/male/pu/own/own_t1_head_material.mtl";
const GENERIC_HEAD: &str =
    "Objects/Characters/Human/heads/male/pu/protos_human_male_face_t1_pu_head.skin";
const GENERIC_EYES: &str =
    "Objects/Characters/Human/heads/male/pu/protos_human_male_face_t1_pu_eyes.skin";
const OWN_EYES: &str = "Objects/Characters/Human/heads/male/pu/own/own_t1_eyes.skin";
const MAIN_EYES: &str = "Objects/Test/main_eyes.cgf";

const STORY_HEAD: &str = "own_t1_head";
const GENERIC_HEAD_CLASS: &str = "PU_Protos_Head";

/// What the synthetic Archive holds besides the Records.
struct Files {
    own_eyes: bool,
    generic_eyes: bool,
}

fn skin(path: &str, model: &TestModel) -> [TestEntry; 2] {
    let stem = path.trim_end_matches(".skin").replace('/', "\\");
    [
        TestEntry::stored(format!("Data\\{stem}.skin"), model.cgf()),
        TestEntry::stored(format!("Data\\{stem}.skinm"), model.cgfm()),
    ]
}

/// A character whose head `head_class` has the material library `library`; its eyes name the
/// generic eyes for the flag `own_head` of the head's port and a main geometry.
fn entries(head_class: &str, library: &str, port_flags: &str, files: &Files) -> Vec<TestEntry> {
    let eyes = TestEntity::new(0x20, "TEST_Eyes", "entities/test_eyes.xml")
        .attach_type("Char_Head_Eyes")
        .geometry(MAIN_EYES, None)
        .geometry_variant("own_head", GENERIC_EYES, None);
    let head = TestEntity::new(0x10, head_class, "entities/test_head.xml")
        .attach_type("Char_Head")
        .sub_type("Male")
        .geometry(GENERIC_HEAD, Some(library))
        .port(
            TestPort::new("Eyes_ItemPort")
                .flags(port_flags)
                .default_item(0x20),
        );
    let npc = TestEntity::new(0x1, "TEST_Npc", "entities/test_npc.xml")
        .port(TestPort::new("Head_ItemPort").default_item(0x10));
    let database = [eyes, head, npc]
        .iter()
        .fold(entity_schema(DataForgeBuilder::new()), |builder, entity| {
            builder.record(entity.record())
        });
    let model = TestModel::cube();
    let mut entries = vec![
        TestEntry::zstd("Data\\Game2.dcb", database.build()),
        TestEntry::stored("Data\\Objects\\Test\\main_eyes.cgf", model.cgf()),
        TestEntry::stored("Data\\Objects\\Test\\main_eyes.cgfm", model.cgfm()),
    ];
    entries.extend(skin(GENERIC_HEAD, &model));
    if files.generic_eyes {
        entries.extend(skin(GENERIC_EYES, &model));
    }
    if files.own_eyes {
        entries.extend(skin(OWN_EYES, &model));
    }
    entries
}

struct Built {
    assembly: Assembly,
}

impl Built {
    fn eyes(&self) -> Option<&str> {
        self.assembly
            .parts()
            .iter()
            .find(|part| part.name().starts_with("Eyes_ItemPort/"))
            .and_then(|part| part.geometry())
    }

    fn codes(&self) -> Vec<&str> {
        self.assembly
            .issues()
            .iter()
            .map(|issue| issue.kind().code())
            .collect()
    }
}

fn build(
    head_class: &str,
    library: &str,
    port_flags: &str,
    files: &Files,
) -> Result<Built, Box<dyn Error>> {
    let fixture = fixture(entries(head_class, library, port_flags, files))?;
    let archive = Archive::open(&fixture.path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find("TEST_Npc")?;
    let tree = resolver.resolve(&record)?;
    let models = ArchiveModels::new(&archive);
    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default())?;
    Ok(Built { assembly })
}

const FLAGS: &str = "own_head";

#[test]
fn a_missing_own_skin_and_a_missing_flag_sub_geometry_end_at_the_main_geometry_with_the_old_note()
-> TestResult {
    let files = Files {
        own_eyes: false,
        generic_eyes: false,
    };
    let built = build(STORY_HEAD, LIBRARY, FLAGS, &files)?;
    assert_eq!(built.eyes(), Some(MAIN_EYES));
    let codes = built.codes();
    assert!(codes.contains(&"sub_geometry_missing"), "{codes:?}");
    assert!(!codes.contains(&"geometry_missing"), "{codes:?}");
    Ok(())
}

#[test]
fn a_missing_own_skin_leaves_the_flag_sub_geometry_without_a_note() -> TestResult {
    let files = Files {
        own_eyes: false,
        generic_eyes: true,
    };
    let built = build(STORY_HEAD, LIBRARY, FLAGS, &files)?;
    assert_eq!(built.eyes(), Some(GENERIC_EYES));
    assert!(!built.codes().contains(&"sub_geometry_missing"));
    Ok(())
}

#[test]
fn the_own_skin_is_drawn_when_the_archive_holds_it_even_without_the_generic_one() -> TestResult {
    let files = Files {
        own_eyes: true,
        generic_eyes: false,
    };
    let built = build(STORY_HEAD, LIBRARY, FLAGS, &files)?;
    assert_eq!(built.eyes(), Some(OWN_EYES));
    assert!(!built.codes().contains(&"sub_geometry_missing"));
    Ok(())
}

#[test]
fn a_generic_head_has_no_own_skin_and_keeps_every_note() -> TestResult {
    let files = Files {
        own_eyes: true,
        generic_eyes: false,
    };
    let built = build(GENERIC_HEAD_CLASS, LIBRARY, FLAGS, &files)?;
    assert_eq!(built.eyes(), Some(MAIN_EYES));
    assert!(built.codes().contains(&"sub_geometry_missing"));
    Ok(())
}

#[test]
fn a_head_library_without_a_folder_gives_no_own_skin() -> TestResult {
    let files = Files {
        own_eyes: true,
        generic_eyes: true,
    };
    let built = build(STORY_HEAD, "own_t1_head_material", FLAGS, &files)?;
    assert_eq!(built.eyes(), Some(GENERIC_EYES));
    Ok(())
}

#[test]
fn a_hidden_part_reads_no_own_skin() -> TestResult {
    let files = Files {
        own_eyes: true,
        generic_eyes: true,
    };
    let built = build(STORY_HEAD, LIBRARY, "invisible own_head", &files)?;
    let eyes = built
        .assembly
        .parts()
        .iter()
        .find(|part| part.name().starts_with("Eyes_ItemPort/"))
        .ok_or("the eyes part")?;
    assert!(eyes.is_hidden());
    assert!(eyes.model().is_none());
    Ok(())
}

#[test]
fn a_tree_without_a_head_is_left_as_it_is() -> TestResult {
    let eyes = TestEntity::new(0x20, "TEST_Eyes", "entities/test_eyes.xml")
        .attach_type("Char_Head_Eyes")
        .geometry(MAIN_EYES, None)
        .geometry_variant("own_head", GENERIC_EYES, None);
    let npc = TestEntity::new(0x1, "TEST_Npc", "entities/test_npc.xml").port(
        TestPort::new("Eyes_ItemPort")
            .flags(FLAGS)
            .default_item(0x20),
    );
    let database = [eyes, npc]
        .iter()
        .fold(entity_schema(DataForgeBuilder::new()), |builder, entity| {
            builder.record(entity.record())
        });
    let model = TestModel::cube();
    let mut entries = vec![TestEntry::zstd(r"Data\Game2.dcb", database.build())];
    entries.extend(skin(GENERIC_EYES, &model));
    entries.extend(skin(OWN_EYES, &model));
    let fixture = fixture(entries)?;
    let archive = Archive::open(&fixture.path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find("TEST_Npc")?;
    let tree = resolver.resolve(&record)?;
    let models = ArchiveModels::new(&archive);
    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default())?;
    let eyes = assembly.part_at("Eyes_ItemPort").ok_or("the eyes part")?;
    assert_eq!(eyes.geometry(), Some(GENERIC_EYES));
    Ok(())
}
