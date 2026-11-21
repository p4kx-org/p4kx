// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The sub-geometry a character's body gives its clothing: what it keeps, what it takes along,
//! what a flag choice means and what a missing file is reported as.

use std::error::Error;

use p4kx_assembly::{
    ArchiveModels, Assembly, AssemblyOptions, EntityNode, ForgePalettes, Geometry, LoadoutResolver,
    PaletteSelection,
};
use p4kx_dataforge::DataForge;
use p4kx_extraction::{Archive, DataForgeEntry, SourceLocation};
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestEntry, TestModel, TestPort, entity_schema,
    tint_palette_record,
};

use crate::assembly_support::{TestResult, fixture};

const MAN: u128 = 0x1000;
const HEADLESS: u128 = 0x1100;
const HEAD: u128 = 0x2000;
const JACKET: u128 = 0x3000;
const VISOR: u128 = 0x3100;
const COAT: u128 = 0x3200;
const TROUSERS: u128 = 0x3300;
const JACKET_PALETTE: u128 = 0x4000;
const BOX: &str = "Objects/Test/box.cgf";
const CUBE: &str = "Objects/Test/cube.cgf";

fn worn(guid: u128, name: &str, item_type: &str) -> TestEntity {
    TestEntity::new(guid, name, format!("entities/{name}.xml")).attach_type(item_type)
}

fn records() -> Vec<TestEntity> {
    let head = worn(HEAD, "TEST_Head", "Char_Head")
        .sub_type("Male")
        .geometry(CUBE, None);
    let jacket = worn(JACKET, "TEST_Jacket", "Char_Clothing_Torso_1")
        .geometry(BOX, Some("Objects/Test/box.mtl"))
        .palette(JACKET_PALETTE, "")
        .geometry_variant("Male", CUBE, None)
        .geometry_variant("Female", "Objects/Test/jacket_f.skin", None);
    let visor = worn(VISOR, "TEST_Visor", "Char_Clothing_Hat")
        .geometry(CUBE, None)
        .geometry_variant("up", CUBE, None)
        .geometry_variant("Male", "Objects/Test/visor_m.skin", None);
    let coat = worn(COAT, "TEST_Coat", "Char_Clothing_Torso_2")
        .geometry(BOX, Some("Objects/Test/box.mtl"))
        .geometry_variant("Male", CUBE, None)
        .material_choice("Male", "Objects/Test/coat_male.mtl");
    let trousers = worn(TROUSERS, "TEST_Trousers", "Char_Clothing_Legs")
        .geometry(BOX, None)
        .geometry_variant("Male", "Objects/Test/absent.cgf", None);
    let man = TestEntity::new(MAN, "TEST_Man", "entities/TEST_Man.xml")
        .port(TestPort::new("head").default_item(HEAD))
        .port(TestPort::new("torso").default_item(JACKET))
        .port(TestPort::new("hat").flags("up").default_item(VISOR))
        .port(TestPort::new("coat").default_item(COAT))
        .port(TestPort::new("legs").default_item(TROUSERS));
    let headless = TestEntity::new(HEADLESS, "TEST_Headless", "entities/TEST_Headless.xml")
        .port(TestPort::new("torso").default_item(JACKET));
    vec![head, jacket, visor, coat, trousers, man, headless]
}

fn database() -> Vec<u8> {
    records()
        .into_iter()
        .fold(entity_schema(DataForgeBuilder::new()), |builder, entity| {
            builder.record(entity.record())
        })
        .record(tint_palette_record(
            JACKET_PALETTE,
            "jacket",
            [[255, 0, 0]; 3],
            [255; 3],
            [200; 3],
        ))
        .build()
}

fn resolve(class: &str) -> Result<EntityNode, Box<dyn Error>> {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let mut resolver = LoadoutResolver::new(&forge);
    let entity = resolver.entities().find(class)?;
    Ok(resolver.resolve(&entity)?)
}

fn child<'n>(node: &'n EntityNode, port: &str) -> Result<&'n EntityNode, Box<dyn Error>> {
    node.ports()
        .iter()
        .find(|candidate| candidate.name() == port)
        .and_then(|found| match found.attachment() {
            p4kx_assembly::Attachment::Entity(entity) => Some(&**entity),
            _ => None,
        })
        .ok_or_else(|| format!("no Entity at {port}").into())
}

#[test]
fn a_character_item_in_a_tree_without_a_head_keeps_its_main_geometry() -> TestResult {
    let tree = resolve("TEST_Headless")?;
    assert!(tree.character_body().is_none());
    let jacket = child(&tree, "torso")?;
    assert_eq!(jacket.geometry().map(Geometry::path), Some(BOX));
    assert_eq!(
        jacket.geometry().and_then(Geometry::material),
        Some("Objects/Test/box.mtl")
    );
    Ok(())
}

#[test]
fn a_body_variant_without_a_material_does_not_take_the_one_of_the_placeholder() -> TestResult {
    let tree = resolve("TEST_Man")?;
    let jacket = child(&tree, "torso")?;
    assert_eq!(jacket.geometry().map(Geometry::path), Some(CUBE));
    assert_eq!(jacket.geometry().and_then(Geometry::material), None);
    Ok(())
}

#[test]
fn a_body_variant_takes_the_material_the_material_tree_has_for_the_body() -> TestResult {
    let tree = resolve("TEST_Man")?;
    let coat = child(&tree, "coat")?;
    assert_eq!(
        coat.geometry().and_then(Geometry::material),
        Some("Objects/Test/coat_male.mtl")
    );
    Ok(())
}

#[test]
fn a_flag_choice_with_the_path_of_the_main_geometry_is_not_overridden_by_the_body() -> TestResult {
    let tree = resolve("TEST_Man")?;
    let visor = child(&tree, "hat")?;
    assert_eq!(visor.geometry().map(Geometry::path), Some(CUBE));
    Ok(())
}

#[test]
fn the_body_and_the_head_that_names_it_are_found_in_the_tree() -> TestResult {
    let tree = resolve("TEST_Man")?;
    let body = tree.character_body().ok_or("no body")?;
    assert_eq!(body.variant().code(), "male");
    assert_eq!(body.head().class_name(), "TEST_Head");
    Ok(())
}

fn assemble_man() -> Result<(Assembly, u32), Box<dyn Error>> {
    let model = TestModel::cube();
    let fixture = fixture(vec![
        TestEntry::zstd("Data\\Game2.dcb", database()),
        TestEntry::stored("Data\\Objects\\Test\\cube.cgf", model.cgf()),
        TestEntry::stored("Data\\Objects\\Test\\cube.cgfm", model.cgfm()),
    ])?;
    let archive = Archive::open(&fixture.path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge).with_forge_entry(&entry);
    let record = resolver.entities().find("TEST_Man")?;
    let tree = resolver.resolve(&record)?;
    let jacket = child(&tree, "torso")?
        .record_index()
        .ok_or("the jacket Record")?;
    let models = ArchiveModels::new(&archive);
    let assembly = Assembly::build_with_palettes(
        &tree,
        &models,
        &ForgePalettes::new(&forge).with_entry(&entry),
        &AssemblyOptions::default(),
    )?;
    Ok((assembly, jacket))
}

#[test]
fn a_body_variant_uses_the_original_main_palette_field_when_unset() -> TestResult {
    let (assembly, jacket) = assemble_man()?;
    let torso = assembly.part_at("torso").ok_or("the worn jacket")?;
    assert_eq!(torso.geometry(), Some(CUBE));
    assert_eq!(
        torso.palette().map(|palette| palette.primary.tint),
        Some([1.0, 0.0, 0.0])
    );
    let source = torso.palette_source().ok_or("the selected main palette")?;
    assert_eq!(source.inherited_from(), None);
    assert!(matches!(
        source.selection(),
        PaletteSelection::Named {
            location: Some(SourceLocation::DataForge { record_index, field })
        } if *record_index == jacket && field == "Components[0].Geometry.Geometry.Palette.RootRecord"
    ));
    Ok(())
}

#[test]
fn a_missing_body_variant_file_is_reported_with_the_body_and_the_path() -> TestResult {
    let (assembly, _) = assemble_man()?;
    let legs: Vec<_> = assembly
        .issues()
        .iter()
        .filter(|issue| issue.path() == "legs")
        .map(|issue| {
            (
                issue.kind().code(),
                issue.kind().skips(),
                issue.kind().to_string(),
            )
        })
        .collect();
    assert_eq!(
        legs,
        vec![(
            "body_geometry_missing",
            true,
            "the male sub-geometry Objects/Test/absent.cgf does not exist".to_owned()
        )]
    );
    Ok(())
}
