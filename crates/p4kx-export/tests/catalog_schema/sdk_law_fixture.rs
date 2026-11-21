// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use super::TestResult;

const INFRACTION_SOURCE: &str = "InfractionDefinition";
const USE_SOURCE: &str = "Infraction";
const SET_SOURCE: &str = "InfractionSet";
const JURISDICTION_SOURCE: &str = "Jurisdiction";
const SECURITY_SOURCE: &str = "SecurityNetworkManifest";
const INFRACTION_GUID: u128 = 1;
const SET_GUID: u128 = 2;

#[derive(Clone, Copy)]
pub(super) enum FixtureMode {
    /// Canonical projection input.
    Canonical,
    /// Input with two physical declarations of a GUID.
    PhysicalAliases,
}

pub(super) fn archive(mode: FixtureMode) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("law-fixture.p4k");
    let mut database = database();
    if matches!(mode, FixtureMode::PhysicalAliases) {
        database = database.record(definition("alias"));
    }
    let dataforge = TestEntry::stored("Data/Game2.dcb", database.build());
    let (dataforge, key) = match mode {
        FixtureMode::Canonical => (dataforge.encrypted(), Some(p4kx_testkit::test_key())),
        FixtureMode::PhysicalAliases => (dataforge, None),
    };
    ArchiveBuilder::new()
        .entry(dataforge)
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            b"test_law_name=Fixture Name\ntest_law_text=Fixture Description\n",
        ))
        .write_to(&path)?;
    Ok((folder, Archive::open_with_key(path, key)?))
}

fn declarations() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(INFRACTION_SOURCE)
                .field("name", TestType::Locale)
                .field("description", TestType::Locale),
        )
        .structure(TestStruct::new(USE_SOURCE).field(
            "definition",
            TestType::ReferenceTo(INFRACTION_SOURCE.to_owned()),
        ))
        .structure(
            TestStruct::new(SET_SOURCE)
                .array("infractions", TestType::Class(USE_SOURCE.to_owned())),
        )
        .structure(
            TestStruct::new(JURISDICTION_SOURCE)
                .array(
                    "infractionSets",
                    TestType::ReferenceTo(SET_SOURCE.to_owned()),
                )
                .field("baseFine", TestType::Int32),
        )
        .structure(TestStruct::new(SECURITY_SOURCE).field("isNeutralTerritory", TestType::Bool))
}

fn database() -> DataForgeBuilder {
    declarations()
        .record(definition("original"))
        .record(
            TestRecord::new(
                SET_GUID,
                SET_SOURCE,
                "FixtureSet",
                "fixture/infraction-set.xml",
            )
            .value(
                "infractions",
                TestValue::Array(
                    [INFRACTION_GUID; 2]
                        .into_iter()
                        .map(|guid| {
                            TestValue::Struct(
                                TestInstance::new(USE_SOURCE)
                                    .value("definition", TestValue::Reference(guid)),
                            )
                        })
                        .collect(),
                ),
            ),
        )
        .record(
            TestRecord::new(
                3,
                JURISDICTION_SOURCE,
                "FixtureJurisdiction",
                "fixture/jurisdiction.xml",
            )
            .value("baseFine", TestValue::Int32(17))
            .value(
                "infractionSets",
                TestValue::Array(vec![TestValue::Reference(SET_GUID)]),
            ),
        )
        .record(
            TestRecord::new(
                4,
                SECURITY_SOURCE,
                "FixtureSecurityNetwork",
                "fixture/security-network.xml",
            )
            .value("isNeutralTerritory", TestValue::Bool(false)),
        )
}

fn definition(label: &str) -> TestRecord {
    TestRecord::new(
        INFRACTION_GUID,
        INFRACTION_SOURCE,
        format!("Fixture.{label}"),
        format!("fixture/{label}.xml"),
    )
    .value("name", TestValue::text("@test_law_name"))
    .value("description", TestValue::text("@test_law_text"))
}
