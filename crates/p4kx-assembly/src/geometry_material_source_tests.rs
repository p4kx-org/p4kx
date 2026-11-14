// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::{DataForge, StructValue, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::MaterialVariant;
use crate::read;

type TestResult = Result<(), Box<dyn Error>>;

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
    let mut source = read::struct_field(tree.root(), "Material").ok_or("the actual root")?;
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
    let variants = MaterialVariant::read(tree.root());
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
    let source = read::struct_field(tree.root(), "Material").ok_or("the actual source root")?;
    let originals: Vec<&StructValue<'_>> = read::struct_items(source, "materialVariants").collect();
    assert_eq!(
        originals.len(),
        count,
        "all source slots are present before projection"
    );
    for (index, original) in originals.iter().enumerate() {
        assert_eq!(read::text(original, "Tags"), format!("tag-{index}"));
    }
    let variants = MaterialVariant::read(tree.root());
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

fn database(children: Vec<TestValue>) -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("MaterialOwner").field(
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
                .array(
                    "materialVariants",
                    TestType::StrongPointer("MaterialNode".to_owned()),
                ),
        )
        .structure(TestStruct::new("MaterialFile").field("path", TestType::String))
        .record(
            TestRecord::new(
                0x7410,
                "MaterialOwner",
                "MaterialOwner.Source",
                "synthetic/material.xml",
            )
            .value("Material", node("root", "", children)),
        )
        .build()
}

fn node(tags: &str, material: &str, children: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("MaterialNode")
            .value("Tags", TestValue::text(tags))
            .value(
                "Material",
                TestValue::Struct(
                    TestInstance::new("MaterialFile").value("path", TestValue::text(material)),
                ),
            )
            .value("materialVariants", TestValue::Array(children)),
    )
}
