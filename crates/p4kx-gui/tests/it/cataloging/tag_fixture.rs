// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, GlobalIniBuilder, TestRecord, TestStruct, TestType, TestValue};

const ROOT: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0001;
const CHILD: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0002;
const DATABASE: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0003;

/// Two global Tags outside the Filter and two manufacturers on opposite sides of it.
pub(crate) fn tag_taxonomy() -> ArchiveBuilder {
    let forge = super::structures()
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .field("legacyGUID", TestType::UInt32)
                .array("children", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(
            TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .record(tag(ROOT, "Root", &[CHILD]))
        .record(tag(CHILD, "Test_Child", &[]))
        .record(
            TestRecord::new(
                DATABASE,
                "TagDatabase",
                "TagDatabase.Roots",
                "synthetic/taxonomy/roots.xml",
            )
            .value("tags", references(&[ROOT])),
        )
        .record(super::manufacturer(1, "Included", "@fixture_description"))
        .record(super::manufacturer(2, "Outside", "@fixture_description"));
    let english = GlobalIniBuilder::new()
        .text("manufacturer_NameIncluded", "Included manufacturer")
        .text("fixture_description", "Synthetic catalog source.");
    let german = GlobalIniBuilder::new()
        .text("manufacturer_NameIncluded", "Ausgewählter Hersteller")
        .text("fixture_description", "Synthetische Katalogquelle.");
    super::archive(&forge, &english, &german)
}

fn tag(guid: u128, name: &str, children: &[u128]) -> TestRecord {
    TestRecord::new(
        guid,
        "Tag",
        format!("Tag.{name}"),
        format!("synthetic/taxonomy/{name}.xml"),
    )
    .value("tagName", TestValue::text(name))
    .value("legacyGUID", TestValue::UInt32(0))
    .value("children", references(children))
}

fn references(guids: &[u128]) -> TestValue {
    TestValue::Array(guids.iter().copied().map(TestValue::Reference).collect())
}
