// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, Filter, OpenOptions};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestContainer, TestElement, TestEntry, TestInstance,
    TestRecord, TestStruct, TestType, TestValue, test_key,
};
use serde_json::Value;

use super::{ANOMALY, ANOMALY_TYPE, JUMP_TYPE, LIVE, LOCAL, LOGICAL, OWNER};
use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::starmap_fixture;

const SYSTEM: &str = r"Data\ObjectContainers\PU\system\syn\synsystem.socpak";
const LONG_CHAIN_CONTAINERS: usize = 33;

#[derive(Clone, Copy)]
pub(super) enum Scenario {
    Operational,
    Duplicate,
    BadCrc,
    DuplicateOuter,
    DuplicateMetadata,
    ContainerCycle,
    InvalidAncestorPose,
    InvalidEndpointPose,
    LongExternalRoutes,
}

pub(super) fn build(filter: Option<&Filter>, scenario: Scenario) -> TestResult<Value> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let archive = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database().build()))
        .entry(TestEntry::stored(
            SYSTEM,
            starmap_fixture::socpak("synsystem.xml", system(scenario).to_xml()),
        ))
        .entry(gate("actual", true, scenario))
        .entry(gate("logical", false, scenario))
        .entry(gate("anomaly", true, Scenario::Operational))
        .entry(gate("untagged", false, scenario));
    let archive = if matches!(scenario, Scenario::DuplicateOuter) {
        archive.entry(gate("actual", true, Scenario::Operational))
    } else {
        archive
    };
    let archive = if matches!(scenario, Scenario::LongExternalRoutes) {
        with_long_chain(archive)
    } else {
        archive
    };
    archive.write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let mut options = common::options();
    options.filter = filter;
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        build_catalog(&archive, options.with_full_provenance())?.json(),
    )?)
}

fn system(scenario: Scenario) -> TestElement {
    let mut children = vec![
        child("actual", LIVE, "jumppoint_a_b"),
        child("actual", LIVE, "jumppoint_a_b_twice"),
        child("logical", LOGICAL, "jumppoint_b_a"),
        child("anomaly", ANOMALY, "marked_anomaly"),
        child("untagged", ANOMALY, "ordinary_anomaly"),
    ];
    match scenario {
        Scenario::ContainerCycle => children.push(external_child(SYSTEM)),
        Scenario::InvalidAncestorPose => {
            children.push(TestElement::new("Child").attribute("pos", "invalid").child(
                starmap_fixture::children(vec![child("actual", LIVE, "hidden")]),
            ));
        }
        Scenario::InvalidEndpointPose => children.push(
            TestElement::new("Child")
                .attribute("pos", "invalid")
                .attribute("starMapRecord", common::guid(LOGICAL)),
        ),
        Scenario::LongExternalRoutes => {
            children.push(external_child(&chain_path(0)));
            children.push(external_child(&chain_path(LONG_CHAIN_CONTAINERS - 1)));
        }
        _ => {}
    }
    TestElement::new("ObjectContainer").child(starmap_fixture::children(children))
}

fn external_child(path: &str) -> TestElement {
    TestElement::new("Child")
        .attribute("external", "1")
        .attribute("name", path)
}

fn chain_path(index: usize) -> String {
    path_for(&format!("chain{index}"))
}

fn with_long_chain(archive: ArchiveBuilder) -> ArchiveBuilder {
    (0..LONG_CHAIN_CONTAINERS).fold(archive, |archive, index| {
        let mut xml = TestElement::new("ObjectContainer");
        let next = if index + 1 < LONG_CHAIN_CONTAINERS {
            external_child(&chain_path(index + 1))
        } else {
            child("logical", LOGICAL, "deep_logical_endpoint")
        };
        xml = xml.child(starmap_fixture::children(vec![next]));
        let descriptor = format!("chain{index}.xml");
        archive.entry(TestEntry::stored(
            chain_path(index),
            starmap_fixture::socpak(&descriptor, xml.to_xml()),
        ))
    })
}

fn child(name: &str, record: u128, label: &str) -> TestElement {
    starmap_fixture::child("1,2,3", Some(record), label)
        .attribute("external", "1")
        .attribute("name", path_for(name))
        .attribute("guid", common::guid(record + 30))
        .attribute("classGuid", common::guid(999))
}

fn path_for(name: &str) -> String {
    format!("Data/ObjectContainers/PU/system/syn/{name}.socpak")
}

fn gate(name: &str, marker: bool, scenario: Scenario) -> TestEntry {
    let mut main = TestElement::new("ObjectContainer");
    let mut members = TestContainer::new();
    if marker {
        main = main.child(
            TestElement::new("ComponentMetadata")
                .child(TestElement::new("EntityComponentJumpPoint")),
        );
        let metadata = TestElement::new("EntityComponentJumpPoint")
            .child(
                TestElement::new("Entity")
                    .attribute("CryGUID", common::guid(LOCAL))
                    .attribute("offsetFromPivot", "0,1,2,3,4,5"),
            )
            .to_xml();
        let root_metadata = "metadata/EntityComponentJumpPoint.xml";
        let member = if name == "actual" {
            format!("{name}/{root_metadata}")
        } else {
            root_metadata.to_owned()
        };
        if matches!(scenario, Scenario::DuplicateMetadata) && name == "actual" {
            members = members.entry(TestEntry::stored(root_metadata, metadata.clone()));
        }
        members = members.entry(TestEntry::stored(member, metadata));
        let sources = fixture_objects(scenario);
        let editor = TestEntry::stored(format!("{name}_editor.xml"), sources.to_xml());
        let editor = if matches!(scenario, Scenario::BadCrc) {
            editor.with_crc32(1)
        } else {
            editor
        };
        members = members.entry(editor);
    }
    TestEntry::stored(
        path_for(name),
        members
            .entry(TestEntry::stored(format!("{name}.xml"), main.to_xml()))
            .build(),
    )
    .encrypted()
}

fn fixture_objects(scenario: Scenario) -> TestElement {
    let object = TestElement::new("Object")
        .attribute("guid", common::guid(LOCAL))
        .attribute("type", "FixtureJump");
    let mut layer = TestElement::new("Layer").child(object.clone());
    if matches!(scenario, Scenario::Duplicate) {
        layer = layer.child(object);
    }
    TestElement::new("ObjectContainer").child(TestElement::new("Layers").child(layer))
}

fn database() -> DataForgeBuilder {
    let database = DataForgeBuilder::new()
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
        .record(
            TestRecord::new(
                OWNER,
                "EntityClassDefinition",
                "EntityClassDefinition.FixtureJump",
                "fixture/gate-operator.xml",
            )
            .value(
                "Components",
                TestValue::Array(vec![
                    TestValue::Struct(TestInstance::new("DataForgeComponentParams")),
                    TestValue::Struct(
                        TestInstance::new("SJumpPointParams")
                            .value("requiredFuel", TestValue::Single(0.0)),
                    ),
                ]),
            ),
        )
        .record(type_record(JUMP_TYPE, "JumpPoint"))
        .record(type_record(ANOMALY_TYPE, "Anomaly"));
    [
        (LIVE, "jump_live", JUMP_TYPE),
        (LOGICAL, "jump_logical", JUMP_TYPE),
        (ANOMALY, "jump_anomaly", ANOMALY_TYPE),
    ]
    .into_iter()
    .fold(database, |database, (id, name, kind)| {
        database.record(
            TestRecord::new(
                id,
                "StarMapObject",
                name,
                format!("libs/foundry/records/starmap/pu/system/syn/{name}.xml"),
            )
            .value("type", TestValue::Reference(kind)),
        )
    })
}

fn type_record(id: u128, value: &str) -> TestRecord {
    TestRecord::new(
        id,
        "StarMapObjectType",
        value,
        "libs/foundry/records/starmap/types.xml",
    )
    .value("name", TestValue::text(value))
}
