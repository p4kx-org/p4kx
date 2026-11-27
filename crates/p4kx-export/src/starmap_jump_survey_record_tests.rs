// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::survey;
use crate::CatalogOptions;
use crate::build::original_input::test_support::{self, TestResult};
use crate::starmap_containers::jump_class::JumpEndpointRecords;

const ENDPOINT: u128 = 1;
const ENDPOINT_TYPE: u128 = 2;
const SOURCE: u128 = 3;

#[test]
fn inbound_references_in_other_record_families_remain_visible() -> TestResult {
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("StarMapObjectType").field("name", TestType::String))
        .structure(
            TestStruct::new("StarMapObject")
                .field("type", TestType::ReferenceTo("StarMapObjectType".into())),
        )
        .structure(
            TestStruct::new("FixtureConfiguration")
                .field("ref", TestType::ReferenceTo("StarMapObject".into())),
        )
        .record(
            TestRecord::new(
                ENDPOINT_TYPE,
                "StarMapObjectType",
                "JumpPoint",
                "libs/foundry/records/starmap/types.xml",
            )
            .value("name", TestValue::text("JumpPoint")),
        )
        .record(
            TestRecord::new(
                ENDPOINT,
                "StarMapObject",
                "fixture",
                "libs/foundry/records/starmap/pu/system/fixture/fixture.xml",
            )
            .value("type", TestValue::Reference(ENDPOINT_TYPE)),
        )
        .record(
            TestRecord::new(
                SOURCE,
                "FixtureConfiguration",
                "outside",
                "fixture/outside.xml",
            )
            .value("ref", TestValue::Reference(ENDPOINT)),
        )
        .build();

    test_support::with_input(&database, |forge, input| {
        let options = CatalogOptions::new(std::time::UNIX_EPOCH);
        let endpoints = JumpEndpointRecords::read(forge, input, &options)?;
        let result = survey(forge, input, &endpoints)?;
        assert_eq!(
            result.read_records, 3,
            "all original Record bodies were read"
        );
        assert_eq!(result.endpoint_records, 1, "the endpoint was identified");
        assert_eq!(
            result.endpoint_references_in_other_records, 1,
            "the exact physical link survives even outside the named families"
        );
        Ok(())
    })
}
