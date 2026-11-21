// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogRecordSelection, MineableEntity, with_mineable_entities,
};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestStruct, TestType, TestValue};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

use super::{DUPLICATE, entity, schema};

const UNRELATED: &str = "FixtureUnrelatedComponent";

#[test]
fn a_broken_component_slot_only_marks_its_own_mineable_incomplete() -> TestResult {
    let database = schema(DataForgeBuilder::new().version(8))
        .structure(
            TestStruct::new(UNRELATED)
                .parent("DataForgeComponentParams")
                .field("fixtureMarker", TestType::Bool),
        )
        .record(entity(DUPLICATE, "FixtureSparse", slots(TestValue::Null)))
        .record(entity(
            DUPLICATE + 1,
            "FixtureBroken",
            slots(TestValue::Raw(
                [u32::MAX.to_le_bytes(), 0_u32.to_le_bytes()].concat(),
            )),
        ));
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    with_mineable_entities(
        &archive,
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult {
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                [1],
                "only the physically unreadable slot makes its owner incomplete"
            );
            assert!(
                snapshot.coverage().unreadable_bodies.is_empty(),
                "a failed component pointer does not make the Record body unreadable"
            );
            let [sparse, broken] = snapshot.rows() else {
                return Err("both original owners remain available".into());
            };
            verify_readable_slots(sparse)?;
            verify_readable_slots(broken)
        },
    )?
}

fn slots(first: TestValue) -> Vec<TestValue> {
    vec![
        first,
        TestValue::Struct(TestInstance::new(UNRELATED)),
        TestValue::Struct(TestInstance::new("MineableParams")),
    ]
}

fn verify_readable_slots(row: &MineableEntity<'_>) -> TestResult {
    assert_eq!(
        row.component_slots.value().copied(),
        Some(3),
        "the original array size remains known"
    );
    let [mining] = row.mining.as_slice() else {
        return Err("the mineable component after the sparse slots remains readable".into());
    };
    assert_eq!(
        mining.source_position.value().copied(),
        Some(2),
        "null, unreadable and unrelated positions do not shift the mining component"
    );
    Ok(())
}
