// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, test_key,
};
use serde_json::{Value, json};

use super::jump_point_configuration;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const OWNER: &str = "EntityClassDefinition";
const BASE: &str = "FixtureJumpComponent";
const COMPONENT: &str = "SJumpPointParams";
const SIZE: &str = "LandingPadSize";
const SIZE_GUID: u128 = 0x1177_0002;
const FIELD: &str = "largestShipSize";

#[test]
fn jump_point_configuration_keeps_size_links_and_physical_component_slots() -> TestResult {
    with_rows(true, |rows| {
        let [resolved, null] = rows else {
            return Err("two physical configuration components".into());
        };
        assert_eq!(
            resolved.pointer("/ownerRecordIndex/value"),
            null.pointer("/ownerRecordIndex/value"),
            "the two slots share their Entity owner"
        );
        assert_eq!(
            resolved.pointer("/componentOrdinal/value"),
            Some(&json!(1)),
            "the nonmatching prefix retains its physical position"
        );
        assert_eq!(
            null.pointer("/componentOrdinal/value"),
            Some(&json!(2)),
            "adjacent matching components retain distinct physical slots"
        );
        assert_eq!(
            resolved.pointer("/largestShipSize/value/targetRecordIndex/value"),
            Some(&json!(0)),
            "the link identifies the actual landing pad size Record"
        );
        assert_eq!(
            resolved.pointer("/largestShipSize/value/targetType/value"),
            Some(&json!(SIZE)),
            "the unpublished definition retains its declared Record type"
        );
        assert_eq!(
            resolved.pointer("/largestShipSize/value/status"),
            Some(&json!("present")),
            "the canonical target is resolved"
        );
        assert_eq!(
            null.get("largestShipSize"),
            Some(&json!({
                "state": "null",
                "value": null,
                "containsEncryptedSource": true
            })),
            "an explicitly null reference retains its marker and state"
        );
        for field in ["requiredFuel", "effectParams", "sourceUsesRefs"] {
            assert!(
                resolved.get(field).is_none(),
                "{field} is outside the selection"
            );
        }
        Ok(())
    })
}

#[test]
fn jump_point_configuration_keeps_an_undeclared_size_distinct_from_null() -> TestResult {
    with_rows(false, |rows| {
        let [row] = rows else {
            return Err("one original configuration with no size declaration".into());
        };
        assert_eq!(
            row.get("largestShipSize"),
            Some(&json!({
                "state": "absent",
                "value": null,
                "containsEncryptedSource": true
            })),
            "the original component proves the field is absent"
        );
        Ok(())
    })
}

fn with_rows(declared: bool, inspect: impl FnOnce(&[Value]) -> TestResult) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database(declared).build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let table = jump_point_configuration::collect(&session.parsed(None)?, scratch.path(), None)?;
    let value = serde_json::to_value(table)?;
    let rows = value
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("jump point configuration rows")?;
    inspect(rows)
}

fn database(declared: bool) -> DataForgeBuilder {
    let mut configuration = TestStruct::new(COMPONENT)
        .parent(BASE)
        .field("requiredFuel", TestType::Single);
    let mut components = vec![TestValue::Struct(TestInstance::new(BASE))];
    if declared {
        configuration = configuration.field(FIELD, TestType::ReferenceTo(SIZE.to_owned()));
        components.push(TestValue::Struct(
            jump().value(FIELD, TestValue::Reference(SIZE_GUID)),
        ));
        components.push(TestValue::Struct(
            jump().value(FIELD, TestValue::Reference(0)),
        ));
    } else {
        components.push(TestValue::Struct(jump()));
    }
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(SIZE).field("fixture", TestType::Bool))
        .structure(
            TestStruct::new(OWNER).array("Components", TestType::StrongPointer(BASE.to_owned())),
        )
        .structure(TestStruct::new(BASE))
        .structure(configuration)
        .record(
            TestRecord::new(SIZE_GUID, SIZE, "FixtureSize", "fixture/landing-pad.xml")
                .value("fixture", TestValue::Bool(true)),
        )
        .record(
            TestRecord::new(
                0x1177_0001,
                OWNER,
                "EntityClassDefinition.FixtureJump",
                "fixture/jump.xml",
            )
            .value("Components", TestValue::Array(components)),
        )
}

fn jump() -> TestInstance {
    TestInstance::new(COMPONENT).value("requiredFuel", TestValue::Single(3.25))
}
