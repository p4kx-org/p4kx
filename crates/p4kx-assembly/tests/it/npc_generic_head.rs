// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A character whose head names no body (the generic head of most NPCs) is dressed by its
//! `Char_Body` Entity; a worn item whose only geometry is a world prop is left out and says so.

use std::error::Error;

use p4kx_assembly::{
    ArchiveModels, Assembly, AssemblyOptions, Attachment, EntityNode, Geometry, LoadoutResolver,
};
use p4kx_dataforge::DataForge;
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry, TestPort, entity_schema};

use crate::assembly_support::{TestResult, fixture};

const BOX: &str = "Objects/buildingsets/human/universal/prop/storage/box_cardboard/box_a.cgf";
const JACKET_MALE: &str = "Objects/Characters/Human/male_v7/clothing/jacket_m.skin";
const JACKET_FEMALE: &str = "Objects/Characters/Human/female_v2/clothing/jacket_f.skin";
const SHIRT_SKIN: &str = "Objects/Characters/Human/male_v7/clothing/shirt.skin";
const SKIN_OUTSIDE: &str = "Objects/Test/vest.skin";
const FEMALE_BODY: &str = "Objects/Characters/Human/female_v2/body/f_body.skin";
const NEUTRAL_BODY: &str = "Objects/Test/body.skin";
const CREATURE_BODY: &str = "Objects/Characters/Creatures/x/x.skin";

const JACKET: u128 = 0x3000;
const SHIRT: u128 = 0x3100;
const UNIFIED: u128 = 0x3200;
const BOXED: u128 = 0x3300;
const VEST: u128 = 0x3400;
const HEAD_GENERIC: u128 = 0x2000;
const HEAD_FEMALE: u128 = 0x2100;
const BODY_MALE: u128 = 0x4000;
const BODY_FLUID: u128 = 0x4100;
const BODY_UNKNOWN: u128 = 0x4200;
const BODY_FEMALE: u128 = 0x4300;

fn worn(guid: u128, name: &str, item_type: &str) -> TestEntity {
    TestEntity::new(guid, name, format!("entities/{name}.xml")).attach_type(item_type)
}

fn character(guid: u128, class: &str, head: u128, body: Option<u128>) -> TestEntity {
    let npc = TestEntity::new(guid, class, format!("entities/{class}.xml"))
        .port(TestPort::new("head").default_item(head))
        .port(TestPort::new("torso").default_item(JACKET))
        .port(TestPort::new("shirt").default_item(SHIRT))
        .port(TestPort::new("unified").default_item(UNIFIED))
        .port(TestPort::new("boxed").default_item(BOXED))
        .port(TestPort::new("vest").default_item(VEST));
    match body {
        Some(body) => npc.port(TestPort::new("body").default_item(body)),
        None => npc,
    }
}

fn records() -> Vec<TestEntity> {
    let jacket = worn(JACKET, "TEST_Jacket", "Char_Clothing_Torso_1")
        .geometry(BOX, None)
        .geometry_variant("Male", JACKET_MALE, None)
        .geometry_variant("Female", JACKET_FEMALE, None);
    let shirt = worn(SHIRT, "TEST_Shirt", "Char_Clothing_Torso_0").geometry(SHIRT_SKIN, None);
    let vest = worn(VEST, "TEST_Vest", "Char_Clothing_Torso_1").geometry(SKIN_OUTSIDE, None);
    let unified = worn(UNIFIED, "TEST_Unified", "Char_Clothing_Torso_2")
        .geometry(BOX, None)
        .geometry_variant("heldEntity", BOX, None);
    let boxed = worn(BOXED, "TEST_Boxed", "Char_Clothing_Legs").geometry(BOX, None);
    let generic = worn(HEAD_GENERIC, "TEST_HeadGeneric", "Char_Head").sub_type("UNDEFINED");
    let female_head = worn(HEAD_FEMALE, "TEST_HeadFemale", "Char_Head")
        .sub_type("Female")
        .geometry("objects/test/head.skin", None);
    let body = |guid, name, sub_type, path| {
        worn(guid, name, "Char_Body")
            .sub_type(sub_type)
            .geometry(path, None)
    };
    vec![
        jacket,
        shirt,
        vest,
        unified,
        boxed,
        generic,
        female_head,
        body(BODY_MALE, "TEST_BodyMale", "Male", FEMALE_BODY),
        body(BODY_FLUID, "TEST_BodyFluid", "Fluid", FEMALE_BODY),
        body(BODY_UNKNOWN, "TEST_BodyUnknown", "Fluid", CREATURE_BODY),
        body(BODY_FEMALE, "TEST_BodyFemale", "Female", NEUTRAL_BODY),
        character(0x1000, "TEST_ByBodySubType", HEAD_GENERIC, Some(BODY_MALE)),
        character(0x1100, "TEST_ByBodyFolder", HEAD_GENERIC, Some(BODY_FLUID)),
        character(0x1200, "TEST_HeadWins", HEAD_FEMALE, Some(BODY_MALE)),
        character(
            0x1300,
            "TEST_UnknownBodyWearer",
            HEAD_GENERIC,
            Some(BODY_UNKNOWN),
        ),
        character(0x1400, "TEST_NoBody", HEAD_GENERIC, None),
        character(0x1500, "TEST_FemaleBody", HEAD_GENERIC, Some(BODY_FEMALE)),
    ]
}

fn database() -> Vec<u8> {
    records()
        .into_iter()
        .fold(entity_schema(DataForgeBuilder::new()), |builder, entity| {
            builder.record(entity.record())
        })
        .build()
}

fn resolve(class: &str) -> Result<EntityNode, Box<dyn Error>> {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let mut resolver = LoadoutResolver::new(&forge);
    let entity = resolver.entities().find(class)?;
    Ok(resolver.resolve(&entity)?)
}

fn geometry_at(node: &EntityNode, port: &str) -> Result<Option<String>, Box<dyn Error>> {
    let found = node
        .ports()
        .iter()
        .find(|candidate| candidate.name() == port)
        .ok_or_else(|| port.to_owned())?;
    match found.attachment() {
        Attachment::Entity(child) => Ok(child.geometry().map(|g| Geometry::path(g).to_owned())),
        other => Err(format!("{port} holds {other:?}").into()),
    }
}

#[test]
fn a_generic_head_leaves_the_body_to_the_sub_type_of_the_body_entity() -> TestResult {
    let tree = resolve("TEST_ByBodySubType")?;
    assert!(tree.character_body().is_none());
    assert_eq!(geometry_at(&tree, "torso")?.as_deref(), Some(JACKET_MALE));
    Ok(())
}

#[test]
fn a_plain_female_body_entity_dresses_the_character_female() -> TestResult {
    let tree = resolve("TEST_FemaleBody")?;
    assert_eq!(geometry_at(&tree, "torso")?.as_deref(), Some(JACKET_FEMALE));
    Ok(())
}

#[test]
fn a_body_entity_with_another_sub_type_is_told_by_the_folder_of_its_model() -> TestResult {
    let tree = resolve("TEST_ByBodyFolder")?;
    assert_eq!(geometry_at(&tree, "torso")?.as_deref(), Some(JACKET_FEMALE));
    Ok(())
}

#[test]
fn a_head_that_names_a_body_decides_before_the_body_entity() -> TestResult {
    let tree = resolve("TEST_HeadWins")?;
    assert_eq!(geometry_at(&tree, "torso")?.as_deref(), Some(JACKET_FEMALE));
    Ok(())
}

#[test]
fn a_character_whose_head_and_body_name_no_body_keeps_every_main_geometry() -> TestResult {
    let tree = resolve("TEST_UnknownBodyWearer")?;
    assert_eq!(geometry_at(&tree, "torso")?.as_deref(), Some(BOX));
    assert_eq!(geometry_at(&tree, "unified")?.as_deref(), Some(BOX));
    assert_eq!(geometry_at(&tree, "boxed")?.as_deref(), Some(BOX));
    Ok(())
}

#[test]
fn a_tree_without_a_body_entity_and_without_a_named_body_is_left_as_it_is() -> TestResult {
    let tree = resolve("TEST_NoBody")?;
    assert_eq!(geometry_at(&tree, "torso")?.as_deref(), Some(BOX));
    assert_eq!(geometry_at(&tree, "boxed")?.as_deref(), Some(BOX));
    Ok(())
}

#[test]
fn worn_items_with_only_a_world_prop_have_no_geometry_once_the_body_is_known() -> TestResult {
    let tree = resolve("TEST_ByBodySubType")?;
    assert_eq!(geometry_at(&tree, "unified")?, None);
    assert_eq!(geometry_at(&tree, "boxed")?, None);
    Ok(())
}

#[test]
fn a_worn_item_with_a_character_model_and_no_variants_keeps_it() -> TestResult {
    let tree = resolve("TEST_ByBodySubType")?;
    assert_eq!(geometry_at(&tree, "shirt")?.as_deref(), Some(SHIRT_SKIN));
    Ok(())
}

#[test]
fn a_skin_file_outside_the_characters_folder_is_no_world_prop() -> TestResult {
    let tree = resolve("TEST_ByBodySubType")?;
    assert_eq!(geometry_at(&tree, "vest")?.as_deref(), Some(SKIN_OUTSIDE));
    Ok(())
}

#[test]
fn the_body_itself_keeps_its_geometry() -> TestResult {
    let tree = resolve("TEST_ByBodySubType")?;
    assert_eq!(geometry_at(&tree, "body")?.as_deref(), Some(FEMALE_BODY));
    Ok(())
}

#[test]
fn a_left_out_world_prop_is_listed_with_its_own_issue_and_the_path_of_the_prop() -> TestResult {
    let fixture = fixture(vec![TestEntry::zstd("Data\\Game2.dcb", database())])?;
    let archive = Archive::open(&fixture.path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find("TEST_ByBodySubType")?;
    let tree = resolver.resolve(&record)?;
    let models = ArchiveModels::new(&archive);

    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default())?;

    let left_out: Vec<_> = assembly
        .issues()
        .iter()
        .filter(|issue| matches!(issue.path(), "unified" | "boxed"))
        .map(|issue| {
            (
                issue.path().to_owned(),
                issue.kind().code(),
                issue.kind().skips(),
                issue.kind().to_string(),
            )
        })
        .collect();
    let reason = format!("the geometry {BOX} is a prop of the world, not a model of the worn item");
    assert_eq!(
        left_out,
        [
            (
                "unified".to_owned(),
                "world_prop_not_worn",
                true,
                reason.clone()
            ),
            ("boxed".to_owned(), "world_prop_not_worn", true, reason),
        ]
    );
    Ok(())
}
