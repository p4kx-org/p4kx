// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::{Durability, HEALTH, finish_health};
use crate::Kind;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_rows::staged::OrderedComponentRows;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::catalog_item_components;
use crate::record_observation::RecordObservation;

#[test]
fn staging_keeps_typed_health_bytes_in_physical_record_and_component_order() -> TestResult {
    let scratch = tempfile::tempdir()?;
    test_support::with_projection(&database().build(), |forge, projection| {
        let projection = projection.for_kind(Kind::Items);
        let mut stage = OrderedComponentRows::new(scratch.path(), None);
        let mut baseline = Vec::new();
        for record in forge.records() {
            let admission = RecordObservation::capture(projection.original(), &record)?;
            let tree = record.tree()?;
            let (_, read, ()) =
                catalog_item_components::read(&tree, &projection, &admission, None, |_, _, _| {
                    Ok(())
                })?;
            let mut durability = read.rows.durability;
            for row in &durability.health {
                baseline.push((row.owner.key(), serde_json::value::to_raw_value(row)?));
            }
            durability.stage_health(&mut stage)?;
            assert!(
                durability.health.is_empty(),
                "completed rows release their fields"
            );
        }
        baseline.sort_by_key(|(key, _)| *key);
        let expected = CatalogTable::new(
            forge,
            &[HEALTH],
            baseline.into_iter().map(|(_, row)| row).collect(),
        );
        let staged = finish_health(stage, forge)?;
        let bytes = serde_json::to_vec(&expected)?;
        assert_eq!(
            staged.len(),
            3,
            "two occurrences on the first Record and one on the second"
        );
        assert!(
            staged.stored_bytes() > 0,
            "actual compressed staging bytes are counted"
        );
        assert_eq!(
            serde_json::to_vec(&staged)?,
            bytes,
            "exact original fields and source uses"
        );
        assert_eq!(
            serde_json::to_vec(&staged)?,
            bytes,
            "replay keeps the same order and bytes"
        );
        Ok(())
    })
}

#[test]
fn empty_health_staging_keeps_source_family_availability_and_writes_no_rows() -> TestResult {
    let scratch = tempfile::tempdir()?;
    for declaration in [HEALTH, "UnrelatedDeclaration"] {
        let bytes = DataForgeBuilder::new()
            .version(8)
            .structure(TestStruct::new(declaration))
            .build();
        let forge = DataForge::open(&bytes)?;
        let expected = CatalogTable::<Durability<'_>>::new(&forge, &[HEALTH], Vec::new());
        let staged = finish_health(OrderedComponentRows::new(scratch.path(), None), &forge)?;
        assert_eq!(
            serde_json::to_vec(&staged)?,
            serde_json::to_vec(&expected)?,
            "empty Health families retain the same declared availability"
        );
        assert_eq!(
            staged.stored_bytes(),
            0,
            "no file is opened for an empty table"
        );
    }
    Ok(())
}

fn database() -> DataForgeBuilder {
    fn health(value: f32) -> TestValue {
        TestValue::Struct(TestInstance::new(HEALTH).value("Health", TestValue::Single(value)))
    }

    fn item(guid: u128, name: &str, components: Vec<TestValue>) -> TestRecord {
        TestRecord::new(
            guid,
            "EntityClassDefinition",
            format!("EntityClassDefinition.{name}"),
            format!("fixture/{name}.xml"),
        )
        .value("Components", TestValue::Array(components))
    }

    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("DataForgeComponentParams".to_owned()),
        ))
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new(HEALTH)
                .parent("DataForgeComponentParams")
                .field("Health", TestType::Single),
        )
        .structure(TestStruct::new("FixtureOther").parent("DataForgeComponentParams"))
        .record(item(
            3,
            "first",
            vec![
                health(40.0),
                TestValue::Struct(TestInstance::new("FixtureOther")),
                health(f32::from_bits(0x7fc0_1234)),
            ],
        ))
        .record(item(1, "second", vec![health(8.0)]))
}
