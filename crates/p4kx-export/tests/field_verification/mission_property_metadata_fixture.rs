// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

pub(crate) fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .enumeration("MissionChoice", ["OPTION_SENTINEL"])
        .structure(TestStruct::new("MetadataAncestor").field("inherited", TestType::String))
        .structure(
            TestStruct::new("MissionPropertyValue_Alpha")
                .parent("MetadataAncestor")
                .field(
                    "owned",
                    TestType::StrongPointer("OwnedMissionBase".to_owned()),
                )
                .field("choice", TestType::Enum("MissionChoice".to_owned()))
                .field("scalar", TestType::Double)
                .field(
                    "reference",
                    TestType::ReferenceTo("ExternalReference".to_owned()),
                )
                .field("weak", TestType::WeakPointer("ExternalWeak".to_owned())),
        )
        .structure(
            TestStruct::new("MissionPropertyValue_SchemaOnly")
                .field("uninstantiated", TestType::Bool),
        )
        .structure(TestStruct::new("OwnedMissionBase"))
        .structure(
            TestStruct::new("OwnedMissionConcrete")
                .parent("OwnedMissionBase")
                .array("leaves", TestType::Class("OwnedLeaf".to_owned())),
        )
        .structure(TestStruct::new("OwnedLeaf").field("flag", TestType::Bool))
        .structure(
            TestStruct::new("ExternalReference").field("doNotFollowReference", TestType::String),
        )
        .structure(TestStruct::new("ExternalWeak").field("doNotFollowWeak", TestType::String))
        .structure(
            TestStruct::new("UnrelatedSibling")
                .parent("MetadataAncestor")
                .field("doNotFollowSibling", TestType::String),
        )
        .record(
            TestRecord::new(
                0x1111,
                "MissionPropertyValue_Alpha",
                "RECORD_NAME_SENTINEL",
                "private/PATH_SENTINEL.xml",
            )
            .value("inherited", TestValue::text("VALUE_SENTINEL"))
            .value("scalar", TestValue::Double(9876.125)),
        )
        .build()
}

pub(crate) fn excessive_closure() -> DataForgeBuilder {
    const CHAIN_LENGTH: u32 = 128;
    let mut builder =
        DataForgeBuilder::new().structure(TestStruct::new("MissionPropertyValue_Deep").field(
            "owned",
            TestType::Class(format!("Z{:03}", CHAIN_LENGTH - 1)),
        ));
    for index in 0..CHAIN_LENGTH {
        let owner = TestStruct::new(format!("Z{index:03}"));
        builder = builder.structure(if index == 0 {
            owner
        } else {
            owner.field("owned", TestType::Class(format!("Z{:03}", index - 1)))
        });
    }
    builder
}
