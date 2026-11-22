// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, DataForgeEntry, SourceLocation};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestLoadout,
    TestLoadoutEntry, TestModel, TestMtl, TestMtlMaterial, TestPort, TestValue, entity_schema,
};

use crate::{ArchiveModels, Assembly, AssemblyOptions, LoadoutResolver, Part};

const MODEL: &str = "Objects/Test/cube.cgf";
const NODE: &str = "Objects/Test/selected.mtl";
const GEOMETRY: &str = "Objects/Test/geometry.mtl";

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn entity_material_drives_rendered_parts_and_keeps_the_winning_source() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let model = TestModel::cube();
    let library = TestMtl::single(&TestMtlMaterial::new("Fixture")).cryxml();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()))
        .entry(TestEntry::stored(format!("Data/{MODEL}"), model.cgf()))
        .entry(TestEntry::stored(
            "Data/Objects/Test/cube.cgfm",
            model.cgfm(),
        ))
        .entry(TestEntry::stored(format!("Data/{NODE}"), library.clone()))
        .entry(TestEntry::stored(format!("Data/{GEOMETRY}"), library))
        .write_to(&path)?;

    let archive = Archive::open(&path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge).with_forge_entry(&entry);
    let entity = resolver.entities().find("PrimaryMaterialRoot")?;
    let tree = resolver.resolve(&entity)?;
    let models = ArchiveModels::new(&archive);
    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default())?;

    check_part(
        &assembly,
        "PrimaryMaterialRoot",
        NODE,
        ".Material.Material.path",
    )?;
    check_part(
        &assembly,
        "ConflictingMaterial",
        NODE,
        ".Material.Material.path",
    )?;
    check_part(
        &assembly,
        "EmptyMaterial",
        GEOMETRY,
        ".Geometry.Geometry.Material.path",
    )?;
    check_part(
        &assembly,
        "GeometryMaterial",
        GEOMETRY,
        ".Geometry.Geometry.Material.path",
    )?;
    Ok(())
}

fn check_part(assembly: &Assembly, class: &str, library: &str, field: &str) -> TestResult {
    let part = assembly
        .parts()
        .iter()
        .find(|part| part.class_name() == class)
        .ok_or("the named Entity is in the rendered Assembly")?;
    assert!(
        part.materials().library().is_some(),
        "{class} has a readable material library, got {:?} with {:?}",
        part.materials(),
        part.material_selector()
    );
    assert_eq!(
        part.materials().target(),
        Some(format!("Data/{library}").as_str()),
        "{class} uses the selected physical library"
    );
    check_location(part, field)
}

fn check_location(part: &Part, expected: &str) -> TestResult {
    let Some(SourceLocation::DataForge {
        record_index: model_record,
        field: model_field,
    }) = part.geometry_selector()
    else {
        return Err("the original model selector is available".into());
    };
    let component = model_field
        .strip_suffix(".Geometry.Geometry.Geometry.path")
        .ok_or("the original model component address")?;
    assert_eq!(
        part.material_selector(),
        Some(SourceLocation::DataForge {
            record_index: model_record,
            field: format!("{component}{expected}"),
        }),
        "{} keeps its winning selector on its physical component",
        part.class_name()
    );
    Ok(())
}

fn database() -> Vec<u8> {
    let root = entity(1, "PrimaryMaterialRoot")
        .component(geometry("", NODE))
        .port(TestPort::new("conflict"))
        .port(TestPort::new("empty"))
        .port(TestPort::new("fallback"))
        .loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("conflict", "ConflictingMaterial"),
            TestLoadoutEntry::class("empty", "EmptyMaterial"),
            TestLoadoutEntry::class("fallback", "GeometryMaterial"),
        ]));
    let conflict = entity(2, "ConflictingMaterial").component(geometry(GEOMETRY, NODE));
    let empty = entity(3, "EmptyMaterial").component(geometry(GEOMETRY, ""));
    let fallback = entity(4, "GeometryMaterial").geometry(MODEL, Some(GEOMETRY));
    entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .record(conflict.record())
        .record(empty.record())
        .record(fallback.record())
        .build()
}

fn entity(guid: u128, class: &str) -> TestEntity {
    TestEntity::new(guid, class, format!("entities/test/{class}.xml"))
}

fn geometry(base: &str, material: &str) -> TestValue {
    let resource = |type_name, path| {
        TestValue::Struct(TestInstance::new(type_name).value("path", TestValue::text(path)))
    };
    let model = TestInstance::new("SGeometryDataParams")
        .value("Geometry", resource("GlobalResourceGeometry", MODEL))
        .value("Material", resource("GlobalResourceMaterial", base));
    let node = TestInstance::new("SGeometryNodeParams").value("Geometry", TestValue::Struct(model));
    let material_node = TestInstance::new("SMaterialNodeParams")
        .value("Material", resource("GlobalResourceMaterial", material));
    TestValue::Struct(
        TestInstance::new("SGeometryResourceParams")
            .value("Geometry", TestValue::Struct(node))
            .value("Material", TestValue::Struct(material_node)),
    )
}
