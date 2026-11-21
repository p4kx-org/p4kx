// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

pub(super) fn database(broken: bool, alias: bool) -> DataForgeBuilder {
    let group = TestStruct::new("ResourceTypeGroup")
        .field("displayName", TestType::Locale)
        .field("description", TestType::Locale)
        .field("defaultThumbnailPath", TestType::String)
        .field("defaultThumbnailPathSVG", TestType::String)
        .array("groups", TestType::Reference)
        .array("resources", TestType::Reference);
    let mut source = DataForgeBuilder::new()
        .version(8)
        .structure(if broken {
            group.declared_size(1)
        } else {
            group
        })
        .structure(
            TestStruct::new("ResourceType")
                .declared_size(1)
                .field("value", TestType::UInt64),
        )
        .structure(
            TestStruct::new("Foreign")
                .declared_size(1)
                .field("value", TestType::UInt64),
        )
        .record(record(1, "ResourceTypeGroup", "child").value("resources", refs(&[3])))
        .record(record(2, "ResourceTypeGroup", "parent").value("groups", refs(&[1])))
        .record(record(3, "ResourceType", "resource-canary"))
        .record(record(4, "Foreign", "foreign-canary"));
    if alias {
        source = source.record(record(1, "ResourceTypeGroup", "alias"));
    }
    source
}

fn record(guid: u128, source_type: &str, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        source_type,
        format!("{source_type}.{name}"),
        format!("libs/foundry/records/resourcetypedatabase/{name}.xml"),
    )
}

fn refs(guids: &[u128]) -> TestValue {
    TestValue::Array(guids.iter().copied().map(TestValue::Reference).collect())
}
