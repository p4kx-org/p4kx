// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestRecord, TestStruct, TestType, TestValue,
};

/// One independent resource with localized text and an explicitly false cargo-validation flag.
pub(crate) fn resources() -> ArchiveBuilder {
    let forge = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("ResourceType")
                .field("displayName", TestType::Locale)
                .field("description", TestType::Locale)
                .field("validateDefaultCargoBox", TestType::Bool),
        )
        .record(
            TestRecord::new(
                0x3000_0000_0000_4000_8000_0000_0000_0001,
                "ResourceType",
                "ResourceType.FixtureResource",
                "libs/foundry/records/resourcetypedatabase/fixture_resource.xml",
            )
            .value("displayName", TestValue::text("@fixture_resource_name"))
            .value(
                "description",
                TestValue::text("@fixture_resource_description"),
            )
            .value("validateDefaultCargoBox", TestValue::Bool(false)),
        );
    let english = GlobalIniBuilder::new()
        .text("fixture_resource_name", "Fixture resource")
        .text(
            "fixture_resource_description",
            "Synthetic catalog resource.",
        );
    let german = GlobalIniBuilder::new()
        .text("fixture_resource_name", "Testressource")
        .text(
            "fixture_resource_description",
            "Synthetische Katalogressource.",
        );
    super::archive(&forge, &english, &german)
}
