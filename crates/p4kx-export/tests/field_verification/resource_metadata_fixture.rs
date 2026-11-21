// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

fn resource_type() -> TestStruct {
    TestStruct::new("ResourceType")
        .parent("DeclarationParent")
        .field("description", TestType::String)
        .field("unit", TestType::Enum("ResourceUnit".to_owned()))
        .field("owned", TestType::Class("OwnedMetadata".to_owned()))
        .array(
            "properties",
            TestType::StrongPointer("ResourceTypeProperties".to_owned()),
        )
        .field(
            "link",
            TestType::ReferenceTo("ExternalReferenceTarget".to_owned()),
        )
        .field(
            "weak",
            TestType::WeakPointer("ExternalWeakTarget".to_owned()),
        )
}

pub(crate) fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .enumeration("ResourceUnit", ["OPTION_VALUE_SENTINEL"])
        .structure(TestStruct::new("DeclarationParent").field("parentField", TestType::String))
        .structure(resource_type())
        .structure(
            TestStruct::new("ResourceTypeGroup")
                .array("members", TestType::ReferenceTo("ResourceType".to_owned())),
        )
        .structure(TestStruct::new("ResourceTypeProperties").field("minimum", TestType::Single))
        .structure(
            TestStruct::new("ConcreteResourceProperty")
                .parent("ResourceTypeProperties")
                .field("details", TestType::Class("PropertyDetails".to_owned())),
        )
        .structure(TestStruct::new("OwnedMetadata").field("flag", TestType::Bool))
        .structure(TestStruct::new("PropertyDetails").field("scale", TestType::Double))
        .structure(
            TestStruct::new("ExternalReferenceTarget")
                .field("doNotTraverseReference", TestType::String),
        )
        .structure(
            TestStruct::new("ExternalWeakTarget").field("doNotTraverseWeak", TestType::String),
        )
        .structure(
            TestStruct::new("UnrelatedSibling")
                .parent("DeclarationParent")
                .field("doNotTraverseSibling", TestType::String),
        )
        .record(
            TestRecord::new(
                0x1111,
                "ResourceType",
                "RECORD_NAME_SENTINEL",
                "private/PATH_SENTINEL.xml",
            )
            .value("description", TestValue::text("RECORD_VALUE_SENTINEL"))
            .value(
                "parentField",
                TestValue::text("LOCALISATION_VALUE_SENTINEL"),
            ),
        )
        .record(
            TestRecord::new(
                0x2222,
                "ResourceTypeProperties",
                "NUMERIC_RECORD_SENTINEL",
                "private/NUMERIC_SENTINEL.xml",
            )
            .value("minimum", TestValue::Single(273.125)),
        )
        .build()
}
