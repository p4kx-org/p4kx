// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use crate::CatalogError;
use crate::catalog_queries::equipment::paints::{PaintCompatibilityStatus, PaintSelection};
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};
use crate::guid_text::CatalogGuid;
use crate::paint::PaintCandidate;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const PAINT_GUID: &str = "00000000-0000-0000-0000-000000000001";
const TAG_PATH: &str = "libs/foundry/records/tagdatabase/tagdatabase.tagdatabase.xml";
const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const ATTACH: &str = "SAttachableComponentParams";
const PAINT: &str = "SItemDefinition";
const PORTS: &str = "SItemPortContainerComponentParams";
const PORT: &str = "SItemPortDef";
const PORT_TYPE: &str = "SItemPortDefTypes";

#[test]
fn complete_paint_fleet_matches_the_exact_query_and_keeps_physical_aliases() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let exact = parsed.read_paint_compatibility(PaintSelection::RecordIndex(0), None)?;
    assert_eq!(exact.status, PaintCompatibilityStatus::Found);
    let paint = exact.row.ok_or("exact Paint compatibility row")?;
    let guid = CatalogGuid(PAINT_GUID.parse()?);
    let candidate = PaintCandidate::from_authored(0, guid, &["Fixture"], &[]);
    let (complete, owners) = parsed.with_paint_matches(None, |matches, index, coverage| {
        assert!(
            coverage,
            "the complete fleet has no unreadable original sources"
        );
        candidate.project_fit(matches, index, None)
    })?;
    assert_eq!(
        serde_json::to_value(complete)?,
        serde_json::to_value(paint)?
    );
    assert_eq!(
        owners
            .iter()
            .map(|owner| owner.record_index)
            .collect::<Vec<_>>(),
        [1, 2],
        "equal nominal GUIDs retain both physical owners"
    );
    assert_eq!(
        owners
            .iter()
            .map(|owner| owner.record_index)
            .collect::<Vec<_>>(),
        exact.ship_record_indices
    );
    let provisional =
        parsed.with_paint_matches(None, |_, _, _| Err::<(), _>(CatalogError::Cancelled));
    assert!(matches!(provisional, Err(CatalogError::Cancelled)));
    Ok(())
}

fn database() -> Vec<u8> {
    [
        TestStruct::new(ENTITY).array("Components", TestType::StrongPointer(COMPONENT.to_owned())),
        TestStruct::new(COMPONENT),
        TestStruct::new(ATTACH)
            .parent(COMPONENT)
            .field("AttachDef", class(PAINT)),
        TestStruct::new(PAINT)
            .field("Type", TestType::String)
            .field("Tags", TestType::String),
        TestStruct::new(PORTS)
            .parent(COMPONENT)
            .array("Ports", class(PORT)),
        TestStruct::new(PORT)
            .field("Name", TestType::String)
            .field("RequiredPortTags", TestType::String)
            .array("Types", class(PORT_TYPE)),
        TestStruct::new(PORT_TYPE).field("Type", TestType::String),
        TestStruct::new("Tag")
            .field("tagName", TestType::String)
            .array("children", TestType::ReferenceTo("Tag".to_owned())),
        TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".to_owned())),
    ]
    .into_iter()
    .fold(
        DataForgeBuilder::new().version(8),
        DataForgeBuilder::structure,
    )
    .record(paint())
    .record(ship("fixture_alpha"))
    .record(ship("fixture_beta"))
    .record(
        TestRecord::new(3, "TagDatabase", "TagDatabase.TagDatabase", TAG_PATH)
            .value("tags", TestValue::Array(vec![TestValue::Reference(4)])),
    )
    .record(
        TestRecord::new(4, "Tag", "Tag.Fixture", TAG_PATH)
            .value("tagName", TestValue::text("Fixture"))
            .value("children", TestValue::Array(Vec::new())),
    )
    .build()
}

fn paint() -> TestRecord {
    let source = node(
        TestInstance::new(PAINT)
            .value("Type", TestValue::text("Paints"))
            .value("Tags", TestValue::text("Fixture")),
    );
    let attachment = node(TestInstance::new(ATTACH).value("AttachDef", source));
    entity(1, "fixture_paint", "scitem/ships/paints", attachment)
}

fn ship(class_name: &str) -> TestRecord {
    let kind = node(TestInstance::new(PORT_TYPE).value("Type", TestValue::text("Paints")));
    let port = node(
        TestInstance::new(PORT)
            .value("Name", TestValue::text("hardpoint_paint"))
            .value("RequiredPortTags", TestValue::text("Fixture"))
            .value("Types", TestValue::Array(vec![kind])),
    );
    let ports = node(TestInstance::new(PORTS).value("Ports", TestValue::Array(vec![port])));
    entity(2, class_name, "spaceships", ports)
}

fn entity(guid: u128, class: &str, path: &str, component: TestValue) -> TestRecord {
    TestRecord::new(
        guid,
        ENTITY,
        format!("EntityClassDefinition.{class}"),
        format!("libs/foundry/records/entities/{path}/{class}.xml"),
    )
    .value("Components", TestValue::Array(vec![component]))
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn node(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}
