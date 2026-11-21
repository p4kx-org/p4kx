// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

pub(super) fn database(broken_resources: bool, broken_groups: bool) -> DataForgeBuilder {
    let resource = resource_schema();
    let group = TestStruct::new("ResourceTypeGroup")
        .array(
            "resources",
            TestType::ReferenceTo("ResourceType".to_owned()),
        )
        .array(
            "groups",
            TestType::ReferenceTo("ResourceTypeGroup".to_owned()),
        );
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(if broken_resources {
            resource.declared_size(1)
        } else {
            resource
        })
        .structure(if broken_groups {
            group.declared_size(1)
        } else {
            group
        })
        .structure(element())
        .structure(
            TestStruct::new("MineableComposition")
                .field("depositName", TestType::Locale)
                .field("minimumDistinctElements", TestType::UInt16)
                .array("compositionArray", TestType::Class("Part".to_owned())),
        )
        .structure(TestStruct::new("Part"))
        .structure(TestStruct::new("Density"))
        .structure(TestStruct::new("Cargo"))
        .structure(TestStruct::new("Property"))
        .structure(TestStruct::new("EntityClassDefinition"))
        .structure(
            TestStruct::new("Foreign")
                .declared_size(1)
                .field("value", TestType::UInt64),
        );
    records(builder)
}

fn resource_schema() -> TestStruct {
    TestStruct::new("ResourceType")
        .field("displayName", TestType::Locale)
        .field("description", TestType::Locale)
        .field("densityType", TestType::StrongPointer("Density".to_owned()))
        .field(
            "refinedVersion",
            TestType::ReferenceTo("ResourceType".to_owned()),
        )
        .field(
            "rttThumbnailEntityClass",
            TestType::ReferenceTo("EntityClassDefinition".to_owned()),
        )
        .field("defaultThumbnailPath", TestType::String)
        .field("defaultThumbnailPathSVG", TestType::String)
        .field("validateDefaultCargoBox", TestType::Bool)
        .field(
            "defaultCargoContainers",
            TestType::StrongPointer("Cargo".to_owned()),
        )
        .array("properties", TestType::StrongPointer("Property".to_owned()))
}

fn element() -> TestStruct {
    let mut element = TestStruct::new("MineableElement").field(
        "resourceType",
        TestType::ReferenceTo("ResourceType".to_owned()),
    );
    for field in [
        "elementInstability",
        "elementResistance",
        "elementOptimalWindowMidpoint",
        "elementOptimalWindowMidpointRandomness",
        "elementOptimalWindowThinness",
        "elementExplosionMultiplier",
        "elementClusterFactor",
    ] {
        element = element.field(field, TestType::Single);
    }
    element
}

fn records(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .record(record(1, "ResourceType", "resourcetypedatabase", "alpha"))
        .record(record(2, "ResourceType", "resourcetypedatabase", "beta"))
        .record(
            record(3, "MineableElement", "mining/mineableelements", "alpha")
                .value("resourceType", TestValue::Reference(1)),
        )
        .record(record(
            4,
            "MineableComposition",
            "mining/rockcompositionpresets",
            "alpha",
        ))
        .record(
            record(
                5,
                "ResourceTypeGroup",
                "resourcetypedatabase",
                "group_alpha",
            )
            .value("resources", TestValue::Array(vec![TestValue::Reference(1)])),
        )
        .record(
            record(6, "ResourceTypeGroup", "resourcetypedatabase", "group_beta")
                .value("resources", TestValue::Array(vec![TestValue::Reference(2)])),
        )
        .record(record(7, "Foreign", "unrelated", "never_read"))
}

fn record(guid: u128, kind: &str, directory: &str, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        kind,
        format!("{kind}.{name}"),
        format!("libs/foundry/records/{directory}/{name}.xml"),
    )
}
