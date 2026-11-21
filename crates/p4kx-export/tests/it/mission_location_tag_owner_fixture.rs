// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::spawns_fixture::{LOCATION_A, TAG_SHIP};

const DATABASE: u128 = 0xb000_0000_0000_4000_8000_0000_0000_00f0;
const DANGLING: u128 = 0xb000_0000_0000_4000_8000_0000_0000_00f1;

#[derive(Clone, Copy, Debug)]
pub(super) enum Ancestor {
    LocationData,
    GeneralTags,
    ProducesTags,
    ConsumesTags,
    AiSpawnTags,
    StringVariants,
}

impl Ancestor {
    pub(super) const ALL: [Self; 6] = [
        Self::LocationData,
        Self::GeneralTags,
        Self::ProducesTags,
        Self::ConsumesTags,
        Self::AiSpawnTags,
        Self::StringVariants,
    ];

    pub(super) const fn member(self) -> &'static str {
        match self {
            Self::LocationData => "locationData",
            Self::GeneralTags => "generalTags",
            Self::ProducesTags => "producesTags",
            Self::ConsumesTags => "consumesTags",
            Self::AiSpawnTags => "aiSpawnTags",
            Self::StringVariants => "stringVariants",
        }
    }

    fn field(self, owner: TestStruct, name: &str, kind: &str) -> TestStruct {
        if name == self.member() {
            owner.array(name, TestType::StrongPointer(kind.to_owned()))
        } else {
            owner.field(name, TestType::Class(kind.to_owned()))
        }
    }

    fn value(self, name: &str, owner: TestInstance) -> TestValue {
        let value = TestValue::Struct(owner);
        if name == self.member() {
            let mut invalid = (u32::MAX - 1).to_le_bytes().to_vec();
            invalid.extend(0_u32.to_le_bytes());
            TestValue::Array(vec![TestValue::Raw(invalid), value])
        } else {
            value
        }
    }
}

pub(super) fn database(ancestor: Ancestor) -> DataForgeBuilder {
    let source = structures(ancestor);
    let path = "libs/foundry/records/tagdatabase/tagdatabase.tagdatabase.xml";
    source
        .record(
            TestRecord::new(DATABASE, "TagDatabase", "TagDatabase.TagDatabase", path).value(
                "tags",
                TestValue::Array(vec![TestValue::Reference(TAG_SHIP)]),
            ),
        )
        .record(
            TestRecord::new(TAG_SHIP, "Tag", "Tag.Leaf", path)
                .value("tagName", TestValue::text("Leaf")),
        )
        .record(
            TestRecord::new(
                LOCATION_A,
                "MissionLocationTemplate",
                "MissionLocationTemplate.Selected",
                "libs/foundry/records/missiondata/pu_locations/templates/selected.xml",
            )
            .value(
                "locationData",
                ancestor.value("locationData", location_data(ancestor)),
            ),
        )
}

fn structures(ancestor: Ancestor) -> DataForgeBuilder {
    let mut location = TestStruct::new("MissionLocationData")
        .field("disabled", TestType::Bool)
        .field("isSecurityNetworkHost", TestType::Bool)
        .field(
            "entityClusterMember",
            TestType::ReferenceTo("Tag".to_owned()),
        );
    for (name, kind) in [
        ("generalTags", "TagList"),
        ("producesTags", "TagSearchTerm"),
        ("consumesTags", "TagSearchTerm"),
        ("aiSpawnTags", "TagList"),
        ("stringVariants", "MissionStringVariants"),
    ] {
        location = ancestor.field(location, name, kind);
    }
    [
        ancestor.field(
            TestStruct::new("MissionLocationTemplate"),
            "locationData",
            "MissionLocationData",
        ),
        location,
        TestStruct::new("Tag")
            .field("tagName", TestType::String)
            .array("children", TestType::ReferenceTo("Tag".to_owned())),
        TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".to_owned())),
        TestStruct::new("TagList").array("tags", TestType::Reference),
        TestStruct::new("TagSearchTerm")
            .array("positiveTags", TestType::Reference)
            .array("negativeTags", TestType::Reference),
        TestStruct::new("MissionStringVariants").array(
            "variants",
            TestType::Class("MissionStringVariant".to_owned()),
        ),
        TestStruct::new("MissionStringVariant")
            .field("tag", TestType::Reference)
            .field("string", TestType::Locale),
    ]
    .into_iter()
    .fold(
        DataForgeBuilder::new().version(8),
        DataForgeBuilder::structure,
    )
}

fn operands() -> TestValue {
    TestValue::Array(vec![
        TestValue::Reference(TAG_SHIP),
        TestValue::Null,
        TestValue::Reference(DANGLING),
    ])
}

fn location_data(ancestor: Ancestor) -> TestInstance {
    let mut value = TestInstance::new("MissionLocationData")
        .value("disabled", TestValue::Bool(false))
        .value("isSecurityNetworkHost", TestValue::Bool(false))
        .value("entityClusterMember", TestValue::Reference(TAG_SHIP));
    for name in ["generalTags", "aiSpawnTags"] {
        value = value.value(
            name,
            ancestor.value(name, TestInstance::new("TagList").value("tags", operands())),
        );
    }
    for name in ["producesTags", "consumesTags"] {
        value = value.value(
            name,
            ancestor.value(
                name,
                TestInstance::new("TagSearchTerm")
                    .value("positiveTags", operands())
                    .value("negativeTags", TestValue::Array(Vec::new())),
            ),
        );
    }
    let variants = [TAG_SHIP, 0, DANGLING]
        .into_iter()
        .map(|tag| {
            TestValue::Struct(
                TestInstance::new("MissionStringVariant")
                    .value("tag", TestValue::Reference(tag))
                    .value("string", TestValue::text("@name")),
            )
        })
        .collect();
    value.value(
        "stringVariants",
        ancestor.value(
            "stringVariants",
            TestInstance::new("MissionStringVariants")
                .value("variants", TestValue::Array(variants)),
        ),
    )
}
