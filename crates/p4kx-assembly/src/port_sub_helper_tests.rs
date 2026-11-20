// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::port::{self, PortDefinitionPosition, PortTransform};

const PORT: &str = "SItemPortDef";
const BONE: &str = "SItemPortDefAttachmentImplementationBone";
const HELPER_NODE: &str = "SItemPortDefHelperNode";
const HELPER: &str = "SItemPortDefHelper";

fn helper(name: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new(HELPER)
            .value("Name", TestValue::text(name))
            .value("ItemOffsetHelperName", TestValue::text("item_origin"))
            .value(
                "Offset",
                TestValue::Struct(TestInstance::new("Offset").value(
                    "Position",
                    TestValue::Struct(
                        TestInstance::new("Vector").value("z", TestValue::Single(0.25)),
                    ),
                )),
            ),
    )
}

fn node(tags: &str, helper: TestValue, children: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new(HELPER_NODE)
            .value("Tags", TestValue::text(tags))
            .value("Helper", helper)
            .value("SubHelpers", TestValue::Array(children)),
    )
}

fn port_fixture() -> Vec<u8> {
    let root = node(
        "",
        helper("spine"),
        vec![
            TestValue::Null,
            node(
                "Heavy Variant",
                TestValue::Null,
                vec![node("Large", helper("spine_low"), Vec::new())],
            ),
            node("Heavy Variant", helper("spine_high"), Vec::new()),
        ],
    );
    DataForgeBuilder::new()
        .structure(
            TestStruct::new(PORT).field("AttachmentImplementation", TestType::Class(BONE.into())),
        )
        .structure(TestStruct::new(BONE).field("Helper", TestType::Class(HELPER_NODE.into())))
        .structure(
            TestStruct::new(HELPER_NODE)
                .field("Tags", TestType::String)
                .field("Helper", TestType::StrongPointer(HELPER.into()))
                .array("SubHelpers", TestType::StrongPointer(HELPER_NODE.into())),
        )
        .structure(
            TestStruct::new(HELPER)
                .field("Name", TestType::String)
                .field("ItemOffsetHelperName", TestType::String)
                .field("Offset", TestType::Class("Offset".into())),
        )
        .structure(TestStruct::new("Offset").field("Position", TestType::Class("Vector".into())))
        .structure(TestStruct::new("Vector").field("z", TestType::Single))
        .record(
            TestRecord::new(1, PORT, "Port.Sample", "ports/sample.xml").value(
                "AttachmentImplementation",
                TestValue::Struct(TestInstance::new(BONE).value("Helper", root)),
            ),
        )
        .build()
}

#[test]
fn the_authored_subhelpers_keep_parentage_and_source_order() -> Result<(), Box<dyn Error>> {
    let bytes = port_fixture();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.records().next().ok_or("sample port")?.tree()?;
    let definition = port::read_definition(
        tree.root(),
        PortDefinitionPosition {
            record: tree.record_index(),
            component: 0,
            port: 0,
        },
    );
    assert_eq!(definition.root_helper_tags(), Some(""));
    assert_eq!(
        definition.transform().map(PortTransform::helper),
        Some("spine")
    );
    let children = definition.sub_helpers();
    assert_eq!(
        children
            .iter()
            .map(|row| (row.parent(), row.ordinal(), row.tags()))
            .collect::<Vec<_>>(),
        [
            (None, 1, Some("Heavy Variant")),
            (Some(0), 0, Some("Large")),
            (None, 2, Some("Heavy Variant")),
        ]
    );
    assert!(children[0].transform().is_none(), "only a tag is authored");
    let low = children[1].transform().ok_or("low helper")?;
    assert_eq!(low.attachment(), "Bone");
    assert_eq!(low.helper(), "spine_low");
    assert_eq!(low.offset_helper(), "item_origin");
    assert_eq!(low.position(), [0.0, 0.0, 0.25]);
    assert_eq!(low.offset_fields(), [true, false]);
    assert_eq!(
        children[2].transform().map(PortTransform::helper),
        Some("spine_high")
    );
    Ok(())
}
