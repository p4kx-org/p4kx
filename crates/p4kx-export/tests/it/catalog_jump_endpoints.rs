// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, Filter, OpenOptions};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestContainer, TestElement, TestEntry, TestRecord,
    TestStruct, TestType, TestValue, test_key,
};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::health_origin_assertions as receipt;
use crate::starmap_fixture;

const ROWS: &str = "/placements/jumpEndpoints/rows";
const JUMP_TYPE: u128 = 101;
const ANOMALY_TYPE: u128 = 102;
const LIVE: u128 = 201;
const LOGICAL: u128 = 202;
const ANOMALY: u128 = 203;
const LOCAL: u128 = 301;

#[test]
fn jump_endpoint_occurrences_keep_same_position_and_do_not_pair_reversed_names() -> TestResult {
    let catalog = build(Some(&Filter::new("jump_")), Scenario::Operational)?;
    let rows = receipt::rows(&catalog, ROWS)?;
    let check = receipt::rows(&catalog, "/placements/starmapPlacements/rows")?;
    if rows.is_empty() {
        return Err(format!(
            "endpoint inventory empty, starmap scope: {:?}; health {}",
            check
                .iter()
                .map(|row| row
                    .pointer("/objectGuid/value")
                    .zip(row.pointer("/position/state")))
                .collect::<Vec<_>>(),
            receipt::at(&catalog, "/exportHealth/status")?
        )
        .into());
    }
    let [first, second, logical, anomaly] = rows else {
        return Err(
            format!("two placements, a logical jump point and one marked Anomaly expected; actual count {} starmap count {} availability {}", rows.len(), receipt::rows(&catalog, "/definitions/starmapObjects/rows")?.len(), receipt::at(&catalog, "/placements/jumpEndpoints/availability")?)
                .into(),
        );
    };
    assert_ne!(
        receipt::at(first, "/instanceId/value")?,
        receipt::at(second, "/instanceId/value")?,
        "same place and record still have separate XML routes"
    );
    assert_eq!(
        receipt::at(first, "/worldPosition/value")?,
        receipt::at(second, "/worldPosition/value")?,
        "position equality has no effect on occurrence identity"
    );
    for row in [first, second, logical, anomaly] {
        receipt::cell(row, "declaredPartner", "absent")?;
    }
    assert_eq!(
        receipt::at(logical, "/gate/value/marker/value")?,
        &json!(false),
        "the source has a logical placement and a readable descriptor without a typed marker"
    );
    assert_eq!(
        receipt::at(anomaly, "/starmapType/value")?,
        "Anomaly",
        "a marked Anomaly is a proven gate occurrence"
    );
    assert_operational(&catalog, first)?;
    Ok(())
}

#[test]
fn jump_endpoint_local_duplicates_and_failed_reads_remain_explicit() -> TestResult {
    let duplicated = build(Some(&Filter::new("jump_live")), Scenario::Duplicate)?;
    let first = live_row(&duplicated)?;
    let [entity] = receipt::rows(first, "/gate/value/entities")? else {
        return Err("one metadata Entity remains".into());
    };
    assert_eq!(
        receipt::rows(entity, "/matches/value")?.len(),
        2,
        "every matching editor Object remains"
    );
    receipt::cell(entity, "object", "invalid")?;
    let unread = build(Some(&Filter::new("jump_live")), Scenario::BadCrc)?;
    let first = live_row(&unread)?;
    assert_eq!(
        receipt::at(first, "/gate/value/editor/state")?,
        "unreadable",
        "CRC failure never becomes a successful or absent source read"
    );
    receipt::cell(first, "declaredPartner", "unreadable")?;
    assert_eq!(
        receipt::at(&unread, "/placements/jumpEndpoints/availability")?,
        "unavailable",
        "the incomplete scope cannot certify its inventory"
    );
    Ok(())
}

fn live_row(catalog: &Value) -> TestResult<&Value> {
    let rows = receipt::rows(catalog, ROWS)?;
    assert_eq!(
        rows.len(),
        2,
        "only the selected DCB Record's physical placements remain"
    );
    rows.first()
        .ok_or_else(|| "native endpoint placement".into())
}

fn assert_operational(catalog: &Value, row: &Value) -> TestResult {
    let [entity] = receipt::rows(row, "/gate/value/entities")? else {
        return Err("the original metadata Entity is retained".into());
    };
    receipt::present(entity, "cryGuid", &json!(common::guid(LOCAL)), Some(true))?;
    receipt::present(entity, "offsetFromPivot", &json!("0,1,2,3,4,5"), Some(true))?;
    let [object] = receipt::rows(entity, "/matches/value")? else {
        return Err("exact same-container editor Object expected".into());
    };
    receipt::present(object, "class", &json!("FixtureJump"), Some(true))?;
    let selection = receipt::cell(entity, "object", "present")?;
    let uses = receipt::uses(catalog, selection)?;
    assert!(
        uses.iter()
            .any(|used| used.pointer("/location/field") == Some(&json!("@CryGUID"))),
        "selection cites its exact source selector"
    );
    assert!(
        uses.iter()
            .any(|used| used.pointer("/location/field") == Some(&json!("@guid"))),
        "the selected target has its own physical source"
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum Scenario {
    Operational,
    Duplicate,
    BadCrc,
}

fn build(filter: Option<&Filter>, scenario: Scenario) -> TestResult<Value> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database().build()))
        .entry(TestEntry::stored(
            r"Data\ObjectContainers\PU\system\syn\synsystem.socpak",
            starmap_fixture::socpak("synsystem.xml", system().to_xml()),
        ))
        .entry(gate("actual", true, scenario))
        .entry(gate("logical", false, scenario))
        .entry(gate("anomaly", true, Scenario::Operational))
        .entry(gate("untagged", false, scenario))
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let mut options = common::options();
    options.filter = filter;
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

fn system() -> TestElement {
    TestElement::new("ObjectContainer").child(starmap_fixture::children(vec![
        child("actual", LIVE, "jumppoint_a_b"),
        child("actual", LIVE, "jumppoint_a_b_twice"),
        child("logical", LOGICAL, "jumppoint_b_a"),
        child("anomaly", ANOMALY, "marked_anomaly"),
        child("untagged", ANOMALY, "ordinary_anomaly"),
    ]))
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
        members = members.entry(TestEntry::stored(
            "metadata/EntityComponentJumpPoint.xml",
            TestElement::new("EntityComponentJumpPoint")
                .child(
                    TestElement::new("Entity")
                        .attribute("CryGUID", common::guid(LOCAL))
                        .attribute("offsetFromPivot", "0,1,2,3,4,5"),
                )
                .to_xml(),
        ));
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
