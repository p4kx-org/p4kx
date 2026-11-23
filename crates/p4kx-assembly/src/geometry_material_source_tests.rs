// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::{DataForge, RecordTree, StructValue, Value};
use p4kx_extraction::SourceLocation;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::MaterialVariant;
use crate::geometry::{GeometryComponent, PaletteReferenceSource};
use crate::read;

type TestResult = Result<(), Box<dyn Error>>;
const PALETTE: u128 = 0x7411;

#[test]
fn every_readable_material_level_survives_the_source_projection() -> TestResult {
    let levels = 9;
    let mut children = Vec::new();
    for level in (1..=levels).rev() {
        children = vec![node(
            &format!("tag-{level}"),
            &format!("materials/{level}.mtl"),
            children,
        )];
    }
    let bytes = database(children);
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("the original material owner")?;
    let tree = record.tree()?;
    assert_eq!(tree.error_count(), 0, "every original node is readable");
    let (component, owner) = geometry_component(&tree)?;
    let mut source = read::struct_field(component, "Material").ok_or("the actual root")?;
    for level in 1..=levels {
        let Some(Value::Array(children)) = source.field("materialVariants") else {
            return Err("the original material child array".into());
        };
        assert_eq!(children.len(), 1, "the exact original child count");
        source = read::as_struct(&children[0]).ok_or("the original child")?;
        assert_eq!(read::text(source, "Tags"), format!("tag-{level}"));
        let material = read::struct_field(source, "Material").ok_or("the source resource")?;
        assert_eq!(
            read::text(material, "path"),
            format!("materials/{level}.mtl")
        );
    }
    assert_eq!(read::struct_items(source, "materialVariants").count(), 0);
    let variants = MaterialVariant::read(component, owner);
    let mut current = variants.as_slice();
    for level in 1..=levels {
        assert_eq!(
            current.len(),
            1,
            "every original child survives at level {level}"
        );
        let variant = &current[0];
        assert_eq!(variant.tags(), [format!("tag-{level}")]);
        assert_eq!(
            variant.material(),
            Some(format!("materials/{level}.mtl").as_str())
        );
        current = variant.children();
    }
    assert!(current.is_empty(), "the actual source terminates here");
    let active: Vec<_> = (1..=levels).map(|level| format!("TAG-{level}")).collect();
    assert_eq!(
        MaterialVariant::resolve(&variants, &active),
        Some("materials/9.mtl")
    );
    Ok(())
}

#[test]
fn all_original_material_siblings_survive_beyond_four_thousand_nodes() -> TestResult {
    let count = 4097;
    let children = (0..count)
        .map(|index| {
            node(
                &format!("tag-{index}"),
                &format!("materials/{index}.mtl"),
                Vec::new(),
            )
        })
        .collect();
    let bytes = database(children);
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("the original material owner")?;
    let tree = record.tree()?;
    assert_eq!(tree.error_count(), 0);
    let (component, owner) = geometry_component(&tree)?;
    let source = read::struct_field(component, "Material").ok_or("the actual source root")?;
    let originals: Vec<&StructValue<'_>> = read::struct_items(source, "materialVariants").collect();
    assert_eq!(
        originals.len(),
        count,
        "all source slots are present before projection"
    );
    for (index, original) in originals.iter().enumerate() {
        assert_eq!(read::text(original, "Tags"), format!("tag-{index}"));
    }
    let variants = MaterialVariant::read(component, owner);
    assert_eq!(variants.len(), count, "no original tail is omitted");
    for (index, variant) in variants.iter().enumerate() {
        assert_eq!(variant.tags(), [format!("tag-{index}")]);
        assert_eq!(
            variant.material(),
            Some(format!("materials/{index}.mtl").as_str())
        );
        assert_eq!(variant.children(), [], "the source sibling has no children");
    }
    assert_eq!(
        MaterialVariant::resolve(&variants, &["tag-4096".to_owned()]),
        Some("materials/4096.mtl")
    );
    Ok(())
}

#[test]
fn a_palette_only_variant_retains_its_original_sparse_position() -> TestResult {
    let bytes = sparse_palette_database();
    let forge = DataForge::open(&bytes)?;
    let tree = forge
        .record(0)
        .ok_or("the original material owner")?
        .tree()?;
    assert_eq!(tree.error_count(), 0);
    let (component, owner) = geometry_component(&tree)?;
    let variants = MaterialVariant::read(component, owner);
    let selected = MaterialVariant::select(&variants, &["LIVERY", "DETAIL"])
        .ok_or("the selected palette-only variant")?;
    let location = |field: &str| SourceLocation::DataForge {
        record_index: tree.record_index(),
        field: field.to_owned(),
    };
    assert_eq!(
        selected.location(),
        location("Components[1].Material.materialVariants[1].materialVariants[1]")
    );
    assert_eq!(selected.material(), None);
    assert_eq!(selected.material_location(), None);
    let palette = selected.palette().ok_or("the original tint palette")?;
    let original = forge.record(1).ok_or("the palette Record")?;
    assert_eq!(palette.guid(), original.guid());
    assert_eq!(palette.child_path(), "synthetic/finish");
    let preferred = palette.candidates().next().ok_or("the preferred palette")?;
    let PaletteReferenceSource::Named(palette_location) = &preferred.selector else {
        return Err("the expected named palette selector".into());
    };
    assert_eq!(
        palette_location.as_ref(),
        Some(&location(
            "Components[1].Material.materialVariants[1].materialVariants[1].Palette.RootRecord"
        ))
    );
    let parent = MaterialVariant::select(&variants, &["LIVERY"]).ok_or("the parent material")?;
    assert_eq!(
        parent.material_location(),
        Some(location(
            "Components[1].Material.materialVariants[1].Material.path"
        ))
    );
    assert_eq!(
        MaterialVariant::resolve(&variants, &["LIVERY".to_owned(), "DETAIL".to_owned()]),
        Some("materials/parent.mtl")
    );
    Ok(())
}

fn sparse_palette_database() -> Vec<u8> {
    let palette_only = material_node("detail", "", Vec::new()).value(
        "Palette",
        TestValue::Struct(
            TestInstance::new("PaletteRef")
                .value("RootRecord", TestValue::Reference(PALETTE))
                .value("ChildPath", TestValue::text("synthetic/finish")),
        ),
    );
    database(vec![
        TestValue::Null,
        node(
            "livery",
            "materials/parent.mtl",
            vec![TestValue::Null, TestValue::Struct(palette_only)],
        ),
        node("livery", "materials/sibling.mtl", Vec::new()),
    ])
}

fn geometry_component<'tree, 'forge>(
    tree: &'tree RecordTree<'forge>,
) -> Result<(&'tree StructValue<'forge>, GeometryComponent), Box<dyn Error>> {
    let (ordinal, component) = read::indexed_struct_items(tree.root(), "Components")
        .next()
        .ok_or("geometry component")?;
    Ok((
        component,
        GeometryComponent {
            record: tree.record_index(),
            ordinal,
        },
    ))
}

fn database(children: Vec<TestValue>) -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("MaterialOwner").array(
            "Components",
            TestType::StrongPointer("GeometryComponent".to_owned()),
        ))
        .structure(TestStruct::new("GeometryComponent").field(
            "Material",
            TestType::StrongPointer("MaterialNode".to_owned()),
        ))
        .structure(
            TestStruct::new("MaterialNode")
                .field("Tags", TestType::String)
                .field(
                    "Material",
                    TestType::StrongPointer("MaterialFile".to_owned()),
                )
                .field("Palette", TestType::StrongPointer("PaletteRef".to_owned()))
                .array(
                    "materialVariants",
                    TestType::StrongPointer("MaterialNode".to_owned()),
                ),
        )
        .structure(TestStruct::new("MaterialFile").field("path", TestType::String))
        .structure(
            TestStruct::new("PaletteRef")
                .field("RootRecord", TestType::ReferenceTo("Palette".to_owned()))
                .field("ChildPath", TestType::String),
        )
        .structure(TestStruct::new("Palette").field("Value", TestType::Int32))
        .record(
            TestRecord::new(
                0x7410,
                "MaterialOwner",
                "MaterialOwner.Source",
                "synthetic/material.xml",
            )
            .value(
                "Components",
                TestValue::Array(vec![
                    TestValue::Null,
                    TestValue::Struct(
                        TestInstance::new("GeometryComponent")
                            .value("Material", node("root", "", children)),
                    ),
                ]),
            ),
        )
        .record(TestRecord::new(
            PALETTE,
            "Palette",
            "Palette.Source",
            "synthetic/palette.xml",
        ))
        .build()
}

fn node(tags: &str, material: &str, children: Vec<TestValue>) -> TestValue {
    TestValue::Struct(material_node(tags, material, children))
}

fn material_node(tags: &str, material: &str, children: Vec<TestValue>) -> TestInstance {
    TestInstance::new("MaterialNode")
        .value("Tags", TestValue::text(tags))
        .value(
            "Material",
            TestValue::Struct(
                TestInstance::new("MaterialFile").value("path", TestValue::text(material)),
            ),
        )
        .value("Palette", TestValue::Null)
        .value("materialVariants", TestValue::Array(children))
}
