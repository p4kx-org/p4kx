// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestContainer, TestEntry, TestInstance, TestRecord,
    TestStruct, TestType, TestValue, test_key,
};

use super::TestResult;

const JUMP_TYPE: u128 = 0x1166_5200;
const ALPHA: u128 = 0x1166_5201;
const BETA: u128 = 0x1166_5202;
const LOCAL: u128 = 0x1166_5203;
const OWNER: u128 = 0x1166_5204;
const UNKNOWN: u128 = 0x1166_5205;
const BAD_TYPE: &str = "UnreadableStarmapTypeFixture";
const TARGET: &str = "Data/ObjectContainers/Fixtures/route_gate.socpak";
const BETA_ROOT: &str = "Data/ObjectContainers/PU/system/beta/betasystem.socpak";
const ALPHA_ROOT: &str = "Data/ObjectContainers/PU/system/alpha/alphasystem.socpak";

#[derive(Clone, Copy)]
pub(super) enum Fixture {
    Complete,
    UnreadableBeta,
    DuplicateTarget,
    UnreadableRecordType,
    ExternalNameMissing,
    ExternalNameEmpty,
    LocalEndpoint,
    MissingMetadata,
    NoRoots,
    NoEndpoints,
}

pub(super) fn open_fixture(fixture: Fixture) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let archive = match fixture {
        Fixture::NoRoots => {
            ArchiveBuilder::new().entry(TestEntry::stored("Data/Game2.dcb", unrelated_database()))
        }
        Fixture::NoEndpoints => ArchiveBuilder::new()
            .entry(TestEntry::stored("Data/Game2.dcb", unrelated_database()))
            .entry(TestEntry::stored(BETA_ROOT, system_root("beta", "", 0))),
        _ => endpoint_archive(fixture)?,
    };
    archive.write_to(&path)?;
    let archive = Archive::open_with(&path, OpenOptions::new().key(Some(test_key())))?;
    Ok((directory, archive))
}

fn endpoint_archive(fixture: Fixture) -> TestResult<ArchiveBuilder> {
    let beta = match fixture {
        Fixture::UnreadableBeta => b"unreadable system container".to_vec(),
        Fixture::UnreadableRecordType => root("beta", UNKNOWN, 1),
        Fixture::ExternalNameMissing => terminal_beta(r#" external="1""#),
        Fixture::ExternalNameEmpty => terminal_beta(r#" external="true" name="""#),
        Fixture::LocalEndpoint => terminal_beta(""),
        _ => root("beta", BETA, 1),
    };
    let mut archive = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database(fixture)?))
        .entry(TestEntry::stored(BETA_ROOT, beta))
        .entry(TestEntry::stored(ALPHA_ROOT, root("alpha", ALPHA, 2)))
        .entry(
            TestEntry::stored(TARGET, gate(!matches!(fixture, Fixture::MissingMetadata)))
                .encrypted(),
        );
    if matches!(fixture, Fixture::DuplicateTarget) {
        archive = archive.entry(TestEntry::stored(TARGET, gate(true)).encrypted());
    }
    Ok(archive)
}

pub(super) fn open_recovered() -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let mut bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", unrelated_database()))
        .entry(TestEntry::stored(BETA_ROOT, root("beta", BETA, 1)))
        .build();
    let missing = bytes
        .windows(4)
        .rposition(|window| window == b"PK\x01\x02")
        .ok_or("second directory record")?;
    bytes
        .get_mut(missing..missing + 4)
        .ok_or("second central signature")?
        .fill(0);
    let end = bytes
        .windows(4)
        .rposition(|window| window == b"PK\x06\x06")
        .ok_or("ZIP64 end")?;
    bytes.truncate(end);
    std::fs::write(&path, bytes)?;
    let archive = Archive::open_with(&path, OpenOptions::new().recover(true))?;
    assert_eq!(
        archive.entries().len(),
        1,
        "the damaged directory loses its original root Entry"
    );
    Ok((directory, archive))
}

fn unrelated_database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("FixtureUnrelated").field("value", TestType::Bool))
        .record(
            TestRecord::new(
                OWNER,
                "FixtureUnrelated",
                "FixtureUnrelated.Untyped",
                "fixture/unrelated.xml",
            )
            .value("value", TestValue::Bool(false)),
        )
        .build()
}

fn jump_schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("StarMapObjectType").field("name", TestType::String))
        .structure(
            TestStruct::new("StarMapObject")
                .field("type", TestType::ReferenceTo("StarMapObjectType".into())),
        )
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("DataForgeComponentParams".into()),
        ))
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new("SJumpPointParams")
                .parent("DataForgeComponentParams")
                .field("requiredFuel", TestType::Single),
        )
}

fn database(fixture: Fixture) -> TestResult<Vec<u8>> {
    let database = jump_schema()
        .record(
            TestRecord::new(
                JUMP_TYPE,
                "StarMapObjectType",
                "JumpPoint",
                "fixture/types.xml",
            )
            .value("name", TestValue::text("JumpPoint")),
        )
        .record(object(ALPHA, "alpha"))
        .record(object(BETA, "beta"))
        .record(
            TestRecord::new(
                OWNER,
                "EntityClassDefinition",
                "EntityClassDefinition.FixtureJump",
                "fixture/gate-operator.xml",
            )
            .value(
                "Components",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new("SJumpPointParams")
                        .value("requiredFuel", TestValue::Single(3.0)),
                )]),
            ),
        );
    if !matches!(fixture, Fixture::UnreadableRecordType) {
        return Ok(database.build());
    }
    let mut database = database
        .structure(TestStruct::new(BAD_TYPE).field("placeholder", TestType::Bool))
        .record(
            TestRecord::new(
                UNKNOWN,
                BAD_TYPE,
                "FixtureUnknown",
                "libs/foundry/records/starmap/pu/unknown.xml",
            )
            .value("placeholder", TestValue::Bool(false)),
        )
        .build();
    let offset = database
        .windows(BAD_TYPE.len())
        .position(|window| window == BAD_TYPE.as_bytes())
        .ok_or("fixture source type string")?;
    *database.get_mut(offset).ok_or("first source type byte")? = 0xff;
    Ok(database)
}

fn object(guid: u128, system: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "StarMapObject",
        format!("FixtureJump.{system}"),
        format!("libs/foundry/records/starmap/pu/system/{system}/jump.xml"),
    )
    .value("type", TestValue::Reference(JUMP_TYPE))
}

fn root(system: &str, guid: u128, occurrences: u32) -> Vec<u8> {
    let child = format!(
        r#"<Child pos="1,2,3" external="1" name="{TARGET}" starMapRecord="00000000-0000-0000-0000-{guid:012x}"/>"#,
    );
    system_root(system, &child, occurrences)
}

fn terminal_beta(attributes: &str) -> Vec<u8> {
    let child = format!(
        r#"<Child pos="1,2,3" starMapRecord="00000000-0000-0000-0000-{BETA:012x}"{attributes}/>"#
    );
    system_root("beta", &child, 1)
}

fn system_root(system: &str, child: &str, occurrences: u32) -> Vec<u8> {
    let mut xml = String::from("<ObjectContainer><ChildObjectContainers>");
    for _ in 0..occurrences {
        xml.push_str(child);
    }
    xml.push_str("</ChildObjectContainers></ObjectContainer>");
    TestContainer::new()
        .entry(TestEntry::stored(
            format!("{system}system.xml"),
            xml.into_bytes(),
        ))
        .entry(TestEntry::stored("empty.soc", Vec::new()))
        .build()
}

fn gate(include_metadata: bool) -> Vec<u8> {
    let base = b"<ObjectContainer><ComponentMetadata><EntityComponentJumpPoint/></ComponentMetadata></ObjectContainer>";
    let metadata = format!(
        r#"<EntityComponentJumpPoint><Entity CryGUID="00000000-0000-0000-0000-{LOCAL:012x}"/></EntityComponentJumpPoint>"#,
    );
    let editor = format!(
        r#"<ObjectContainer><Layers><Layer><Object guid="00000000-0000-0000-0000-{LOCAL:012x}" type="FixtureJump"/></Layer></Layers></ObjectContainer>"#,
    );
    let mut container =
        TestContainer::new().entry(TestEntry::stored("route_gate.xml", base.as_slice()));
    if include_metadata {
        container = container.entry(TestEntry::stored(
            "metadata/EntityComponentJumpPoint.xml",
            metadata.into_bytes(),
        ));
    }
    container
        .entry(TestEntry::stored(
            "route_gate_editor.xml",
            editor.into_bytes(),
        ))
        .build()
}
