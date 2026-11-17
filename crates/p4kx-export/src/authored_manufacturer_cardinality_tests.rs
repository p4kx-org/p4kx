// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::AuthoredManufacturer;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_v3::prepared_definition::PreparedDefinition;
use crate::record_observation::RecordObservation;

#[test]
fn a_manufacturer_reference_array_retains_each_authored_slot() -> TestResult {
    let data = DataForgeBuilder::new()
        .structure(TestStruct::new("FixtureOwner").array(
            "manufacturer",
            TestType::ReferenceTo("SCItemManufacturer".to_owned()),
        ))
        .structure(TestStruct::new("SCItemManufacturer"))
        .record(
            TestRecord::new(1, "FixtureOwner", "FixtureOwner.Ship", "ship.xml").value(
                "manufacturer",
                TestValue::Array(vec![
                    TestValue::Reference(2),
                    TestValue::Null,
                    TestValue::Reference(2),
                ]),
            ),
        )
        .record(TestRecord::new(
            2,
            "SCItemManufacturer",
            "SCItemManufacturer.Vendor",
            "vendor.xml",
        ))
        .build();
    test_support::with_projection(&data, |forge, projection| {
        let record = forge.record(0).ok_or("original manufacturer owner")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(projection.original(), &record)?;
        let (row, notes) = PreparedDefinition::read(
            &tree,
            projection,
            &admission,
            None,
            |_, fields, _, notes| AuthoredManufacturer::read(fields, "manufacturer", notes),
        )?;
        assert!(notes.is_empty(), "authored array is not a scalar violation");
        let projected = serde_json::to_value(row.into_definition().into_value())?;
        let arena = projected
            .pointer("/value/authored")
            .ok_or("authored array arena")?;
        let elements = arena
            .pointer("/values/0/value/value/elements")
            .and_then(Value::as_array)
            .ok_or("original ordered array slots")?;
        assert_eq!(elements.len(), 3, "duplicate and null slots survive");
        for (ordinal, expected) in [
            (0, json!("00000000-0000-0000-0000-000000000002")),
            (1, Value::Null),
            (2, json!("00000000-0000-0000-0000-000000000002")),
        ] {
            let index: usize = elements[ordinal]["valueIndex"]
                .as_str()
                .ok_or("original value index")?
                .parse()?;
            let slot = &arena["values"][index]["value"];
            if expected.is_null() {
                assert_eq!(slot["state"], "null", "the authored null remains in place");
            } else {
                assert_eq!(
                    slot.pointer("/value/value/guid/value"),
                    Some(&expected),
                    "stored reference survives in its original slot"
                );
            }
        }
        Ok(())
    })
}
