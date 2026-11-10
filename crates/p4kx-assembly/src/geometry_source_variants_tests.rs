// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::{DataForge, StructValue, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::Geometry;
use crate::read;

type TestResult = Result<(), Box<dyn Error>>;
const LEVELS: usize = 7;

#[test]
fn all_subgeometry_levels_keep_breadth_first_order_and_inherited_tags() -> TestResult {
    complete(None)
}

#[test]
fn a_geometryless_parent_keeps_its_tags_and_readable_descendants() -> TestResult {
    complete(Some(2))
}

fn complete(missing_geometry: Option<usize>) -> TestResult {
    let bytes = database(missing_geometry);
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("the original geometry owner")?;
    let tree = record.tree()?;
    assert_eq!(
        tree.error_count(),
        0,
        "all original geometry slots are valid"
    );
    original(tree.root(), missing_geometry)?;
    let variants = Geometry::read_variants(tree.root());
    let expected: Vec<_> = (1..=LEVELS)
        .flat_map(|level| ["a", "b"].map(move |branch| (branch, level)))
        .filter(|(branch, level)| *branch != "a" || Some(*level) != missing_geometry)
        .collect();
    assert_eq!(
        variants.len(),
        expected.len(),
        "every authored model survives"
    );
    for (variant, (branch, level)) in variants.iter().zip(expected) {
        assert_eq!(
            variant.geometry().path(),
            format!("models/{branch}-{level}.cgf")
        );
        let tags: Vec<_> = (1..=level).map(|at| format!("{branch}-{at}")).collect();
        assert_eq!(variant.tags(), tags, "the exact parent tag chain survives");
    }
    Ok(())
}

fn original(component: &StructValue<'_>, missing_geometry: Option<usize>) -> TestResult {
    let root = read::struct_field(component, "Geometry").ok_or("original geometry root")?;
    let Some(Value::Array(children)) = root.field("SubGeometry") else {
        return Err("the original child array".into());
    };
    assert_eq!(
        children.len(),
        3,
        "two source branches surround a null slot"
    );
    assert!(
        matches!(children.get(1), Some(Value::Null)),
        "the original null slot exists"
    );
    for (index, branch) in [(0, "a"), (2, "b")] {
        let Some(Value::Struct(node)) = children.get(index) else {
            return Err("the actual original branch".into());
        };
        original_branch(
            node,
            branch,
            if branch == "a" {
                missing_geometry
            } else {
                None
            },
        )?;
    }
    Ok(())
}

fn original_branch(mut node: &StructValue<'_>, branch: &str, missing: Option<usize>) -> TestResult {
    for level in 1..=LEVELS {
        let Some(Value::String(tags)) = node.field("Tags") else {
            return Err("the original tag token".into());
        };
        assert_eq!(*tags, format!("{branch}-{level}"));
        let geometry = read::struct_field(node, "Geometry");
        if Some(level) == missing {
            assert!(
                geometry.is_none(),
                "the source explicitly lacks this geometry"
            );
        } else {
            let resource = geometry
                .and_then(|data| read::struct_field(data, "Geometry"))
                .ok_or("the actual original model resource")?;
            assert_eq!(
                read::text(resource, "path"),
                format!("models/{branch}-{level}.cgf")
            );
        }
        let Some(Value::Array(children)) = node.field("SubGeometry") else {
            return Err("the actual original child declaration".into());
        };
        if level == LEVELS {
            assert!(children.is_empty(), "the complete source chain terminates");
        } else {
            assert_eq!(children.len(), 1, "the exact original successor count");
            let Some(Value::Struct(child)) = children.first() else {
                return Err("the actual original successor".into());
            };
            node = child;
        }
    }
    Ok(())
}

fn database(missing_geometry: Option<usize>) -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("GeometrySource").field(
            "Geometry",
            TestType::StrongPointer("GeometryNode".to_owned()),
        ))
        .structure(
            TestStruct::new("GeometryNode")
                .field("Tags", TestType::String)
                .field(
                    "Geometry",
                    TestType::StrongPointer("GeometryData".to_owned()),
                )
                .array(
                    "SubGeometry",
                    TestType::StrongPointer("GeometryNode".to_owned()),
                ),
        )
        .structure(TestStruct::new("GeometryData").field(
            "Geometry",
            TestType::StrongPointer("GeometryFile".to_owned()),
        ))
        .structure(TestStruct::new("GeometryFile").field("path", TestType::String))
        .record(
            TestRecord::new(
                0x7401,
                "GeometrySource",
                "GeometrySource.Owner",
                "synthetic/geometry.xml",
            )
            .value(
                "Geometry",
                TestValue::Struct(
                    TestInstance::new("GeometryNode")
                        .value("Tags", TestValue::text("root-is-not-inherited"))
                        .value(
                            "SubGeometry",
                            TestValue::Array(vec![
                                chain("a", missing_geometry),
                                TestValue::Null,
                                chain("b", None),
                            ]),
                        ),
                ),
            ),
        )
        .build()
}

fn chain(branch: &str, missing_geometry: Option<usize>) -> TestValue {
    let mut children = Vec::new();
    for level in (1..=LEVELS).rev() {
        let mut node = TestInstance::new("GeometryNode")
            .value("Tags", TestValue::text(format!("{branch}-{level}")))
            .value("SubGeometry", TestValue::Array(children));
        if Some(level) != missing_geometry {
            node = node.value(
                "Geometry",
                TestValue::Struct(TestInstance::new("GeometryData").value(
                    "Geometry",
                    TestValue::Struct(TestInstance::new("GeometryFile").value(
                        "path",
                        TestValue::text(format!("models/{branch}-{level}.cgf")),
                    )),
                )),
            );
        }
        children = vec![TestValue::Struct(node)];
    }
    children
        .pop()
        .expect("the positive source depth constructs a branch")
}
