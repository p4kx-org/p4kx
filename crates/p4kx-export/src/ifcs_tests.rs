// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, Filter};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::{Value, json};

use crate::{CatalogOptions, CatalogPublicationMode, build_catalog};

const OWNER: &str = "EntityClassDefinition";
const BASE: &str = "FixtureComponent";
const IFCS: &str = "IFCSParams";
const BURN: &str = "FixtureAfterburner";
const CURVE: &str = "FixtureCurve";
const POINT: &str = "Vec2";
const PATH: &str = "fixtures/flight/control";

#[test]
fn catalog_exposes_ifcs_independently_of_specialist_item_selection()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("Data.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("Data/Game2.dcb", fixture().build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let filter = Filter::new(PATH);
    let matching = output(&archive, Some(&filter))?;
    let full = output(&archive, None)?;
    assert_eq!(
        matching
            .pointer("/configurations/ifcs/rows")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "matched IFCS table"
    );
    let row = matching
        .pointer("/configurations/ifcs/rows/0")
        .ok_or("physical IFCS configuration")?;
    assert_eq!(Some(row), full.pointer("/configurations/ifcs/rows/0"));
    check_original_values(row);
    assert_eq!(
        output(&archive, Some(&Filter::new("elsewhere")))?.pointer("/configurations/ifcs/rows"),
        Some(&json!([])),
    );
    Ok(())
}

fn check_original_values(row: &Value) {
    assert_eq!(row.pointer("/ownerRecordIndex/value"), Some(&json!(0)));
    assert_eq!(row.pointer("/componentOrdinal/value"), Some(&json!(1)));
    assert_eq!(
        row.pointer("/limits/scmSpeed/value/rawBits"),
        Some(&json!("80000000"))
    );
    assert_eq!(
        row.pointer("/limits/maxSpeed/value/rawBits"),
        Some(&json!("7fc01234"))
    );
    assert_eq!(
        row.pointer("/limits/maxSpeed/value/finite"),
        Some(&Value::Null)
    );
    assert_eq!(
        row.pointer("/limits/boostSpeedForward/state"),
        Some(&json!("absent"))
    );
    assert_eq!(
        row.pointer("/limits/scmSpeed/containsEncryptedSource"),
        Some(&json!(false))
    );
    assert_eq!(
        row.pointer("/limits/scmSpeed/sourceSetComplete"),
        Some(&json!(true))
    );
    assert_eq!(
        row.pointer("/dynamics/afterburner/value/capacitorMax/value/finite"),
        Some(&json!(10.0))
    );
    assert_eq!(
        row.pointer("/dynamics/afterburnerNew/value/capacitorMax/value/finite"),
        Some(&json!(20.0))
    );
    assert_eq!(
        row.pointer("/dynamics/afterburnerNew/value/afterburnerRampMapping/value/points/value/1/value/x/value/finite"),
        Some(&json!(0.75)),
    );
    assert_eq!(
        row.pointer("/additionalFields/roots/0/name/value"),
        Some(&json!("fixtureUnknown"))
    );
}

fn output(archive: &Archive, filter: Option<&Filter>) -> Result<Value, Box<dyn std::error::Error>> {
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.filter = filter;
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        build_catalog(archive, &options)?.json(),
    )?)
}

fn fixture() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(BASE))
        .structure(
            TestStruct::new(OWNER).array("Components", TestType::StrongPointer(BASE.to_owned())),
        )
        .structure(
            TestStruct::new(IFCS)
                .parent(BASE)
                .field("scmSpeed", TestType::Single)
                .field("maxSpeed", TestType::Single)
                .field("afterburner", TestType::Class(BURN.to_owned()))
                .field("afterburnerNew", TestType::Class(BURN.to_owned()))
                .field("fixtureUnknown", TestType::Bool),
        )
        .structure(
            TestStruct::new(BURN)
                .field("capacitorMax", TestType::Single)
                .field("afterburnerRampMapping", TestType::Class(CURVE.to_owned())),
        )
        .structure(
            TestStruct::new(CURVE)
                .field("useLUT", TestType::Bool)
                .array("points", TestType::Class(POINT.to_owned())),
        )
        .structure(
            TestStruct::new(POINT)
                .field("x", TestType::Single)
                .field("y", TestType::Single),
        )
        .record(
            TestRecord::new(1, OWNER, "FixtureEntity", "fixtures/flight/control.xml").value(
                "Components",
                TestValue::Array(vec![TestValue::Null, ifcs()]),
            ),
        )
}

fn ifcs() -> TestValue {
    TestValue::Struct(
        TestInstance::new(IFCS)
            .value("scmSpeed", TestValue::Single(-0.0))
            .value("maxSpeed", TestValue::Single(f32::from_bits(0x7fc0_1234)))
            .value("afterburner", burn(10.0))
            .value("afterburnerNew", burn(20.0))
            .value("fixtureUnknown", TestValue::Bool(true)),
    )
}

fn burn(capacity: f32) -> TestValue {
    let points = TestValue::Array(vec![point(0.25, 1.0), point(0.75, 2.0)]);
    let curve = TestInstance::new(CURVE)
        .value("useLUT", TestValue::Bool(false))
        .value("points", points);
    TestValue::Struct(
        TestInstance::new(BURN)
            .value("capacitorMax", TestValue::Single(capacity))
            .value("afterburnerRampMapping", TestValue::Struct(curve)),
    )
}

fn point(x: f32, y: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(POINT)
            .value("x", TestValue::Single(x))
            .value("y", TestValue::Single(y)),
    )
}
