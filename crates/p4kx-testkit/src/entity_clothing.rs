// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{TestInstance, TestRecord, TestStruct, TestType, TestValue};

const VISIBILITY_TAG_GUID: u128 = 0x77;

pub(crate) fn visibility_condition_tag() -> TestRecord {
    TestRecord::new(
        VISIBILITY_TAG_GUID,
        "Tag",
        "TEST_ClothingVisibility",
        "libs/foundry/records/tags/test_clothing_visibility.xml",
    )
}

/// The visibility condition of a [`TestZoneChunk`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestClothingCondition {
    /// No condition.
    None,
    /// A condition whose entry names no port and no tag, so it restricts nothing.
    Unrestricted,
    /// A condition with no entries at all.
    NoEntries,
    /// A condition on the port `backpack`.
    OnPort,
    /// A condition on a tag.
    OnTag,
}

/// One zone of the body a worn Entity covers, as the `Chunks` of `SCItemClothingParams` hold it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestZoneChunk {
    /// The zone's name, e.g. `torso01_zone`.
    pub zone: String,
    /// The layer the zone is covered on; the body is on 0.
    pub layer: i32,
    /// The lower layers that stay visible through the chunk.
    pub visible_layers: Vec<i32>,
    /// The chunk's visibility condition.
    pub condition: TestClothingCondition,
}

impl TestZoneChunk {
    /// Creates a chunk of `zone` on `layer` that lets nothing below show through.
    #[must_use]
    pub fn new(zone: impl Into<String>, layer: i32) -> Self {
        Self {
            zone: zone.into(),
            layer,
            visible_layers: Vec::new(),
            condition: TestClothingCondition::None,
        }
    }

    /// Lets the `layers` below show through the chunk.
    #[must_use]
    pub fn showing(mut self, layers: &[i32]) -> Self {
        self.visible_layers = layers.to_vec();
        self
    }

    /// Gives the chunk a visibility condition on the port `backpack`.
    #[must_use]
    pub const fn conditional(mut self) -> Self {
        self.condition = TestClothingCondition::OnPort;
        self
    }

    /// Gives the chunk a visibility condition on a tag.
    #[must_use]
    pub const fn conditional_on_tag(mut self) -> Self {
        self.condition = TestClothingCondition::OnTag;
        self
    }

    /// Gives the chunk a visibility condition that names no port and no tag.
    #[must_use]
    pub const fn unrestricted(mut self) -> Self {
        self.condition = TestClothingCondition::Unrestricted;
        self
    }

    /// Gives the chunk a visibility condition that has no `PortVisibility` entries.
    #[must_use]
    pub const fn without_entries(mut self) -> Self {
        self.condition = TestClothingCondition::NoEntries;
        self
    }

    fn conditions(&self) -> Vec<TestValue> {
        let (ports, tags) = match self.condition {
            TestClothingCondition::None => return Vec::new(),
            TestClothingCondition::NoEntries => {
                return vec![TestValue::Struct(
                    TestInstance::new("SCItemClothingVisibilityConditionParams")
                        .value("PortVisibility", TestValue::Array(Vec::new())),
                )];
            }
            TestClothingCondition::Unrestricted => (Vec::new(), Vec::new()),
            TestClothingCondition::OnPort => (vec![TestValue::text("backpack")], Vec::new()),
            TestClothingCondition::OnTag => {
                (Vec::new(), vec![TestValue::Reference(VISIBILITY_TAG_GUID)])
            }
        };
        let entry = TestInstance::new("SCItemClothingPortVisibilityConditionParams")
            .value("Ports", TestValue::Array(ports))
            .value("DisableTags", TestValue::Array(Vec::new()))
            .value("SkipTags", TestValue::Array(Vec::new()))
            .value("ActiveTags", TestValue::Array(tags));
        vec![TestValue::Struct(
            TestInstance::new("SCItemClothingVisibilityConditionParams").value(
                "PortVisibility",
                TestValue::Array(vec![TestValue::Struct(entry)]),
            ),
        )]
    }

    fn value(&self) -> TestValue {
        let conditions = self.conditions();
        TestValue::Struct(
            TestInstance::new("SCItemClothingMeshChunkParams")
                .value("MeshChunk", TestValue::text(self.zone.clone()))
                .value("Layer", TestValue::Int32(self.layer))
                .value(
                    "VisibleLayers",
                    TestValue::Array(
                        self.visible_layers
                            .iter()
                            .map(|layer| TestValue::Int32(*layer))
                            .collect(),
                    ),
                )
                .value("VisibilityConditions", TestValue::Array(conditions)),
        )
    }
}

/// Returns the `SCItemClothingParams` component that covers `chunks`; add it to an Entity with
/// [`TestEntity::component`](crate::TestEntity::component).
#[must_use]
pub fn clothing_component(chunks: &[TestZoneChunk]) -> TestValue {
    clothing_component_hiding(chunks, &[])
}

/// Returns the `SCItemClothingParams` component that covers `chunks` and hides the items at the
/// ports `hidden` (`HiddenParts`).
#[must_use]
pub(crate) fn clothing_component_hiding(chunks: &[TestZoneChunk], hidden: &[&str]) -> TestValue {
    let parts = hidden
        .iter()
        .map(|port| {
            TestValue::Struct(
                TestInstance::new("SCItemClothingHiddenPartsParams")
                    .value("PortName", TestValue::text(*port)),
            )
        })
        .collect();
    TestValue::Struct(
        TestInstance::new("SCItemClothingParams")
            .value("HiddenParts", TestValue::Array(parts))
            .value(
                "Chunks",
                TestValue::Array(chunks.iter().map(TestZoneChunk::value).collect()),
            ),
    )
}

pub(crate) fn clothing_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("SCItemClothingParams")
            .parent("DataForgeComponentParams")
            .array(
                "HiddenParts",
                TestType::Class("SCItemClothingHiddenPartsParams".to_owned()),
            )
            .array(
                "Chunks",
                TestType::Class("SCItemClothingMeshChunkParams".to_owned()),
            ),
        TestStruct::new("SCItemClothingHiddenPartsParams").field("PortName", TestType::String),
        TestStruct::new("SCItemClothingMeshChunkParams")
            .field("MeshChunk", TestType::String)
            .field("Layer", TestType::Int32)
            .array("VisibleLayers", TestType::Int32)
            .array(
                "VisibilityConditions",
                TestType::Class("SCItemClothingVisibilityConditionParams".to_owned()),
            ),
        TestStruct::new("SCItemClothingVisibilityConditionParams").array(
            "PortVisibility",
            TestType::Class("SCItemClothingPortVisibilityConditionParams".to_owned()),
        ),
        TestStruct::new("SCItemClothingPortVisibilityConditionParams")
            .array("Ports", TestType::String)
            .array("DisableTags", TestType::Reference)
            .array("SkipTags", TestType::Reference)
            .array("ActiveTags", TestType::Reference),
    ]
}
