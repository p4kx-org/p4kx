// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::mining::{with_mining_global_params, with_refining_processes};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_extraction::SourceValueState;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn absent_and_null_owned_blocks_are_not_fabricated_as_empty_present_structs() -> TestResult {
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("MiningGlobalParams").field(
            "mineableInstabilityParams",
            TestType::StrongPointer("ObservedNullable".to_owned()),
        ))
        .structure(TestStruct::new("ObservedNullable"))
        .record(TestRecord::new(
            1,
            "MiningGlobalParams",
            "MiningGlobalParams.Empty",
            "libs/foundry/records/mining/miningglobalparams_empty.xml",
        ));
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    with_mining_global_params(&archive, CatalogRecordSelection::All, None, |snapshot| {
        let row = &snapshot.rows()[0];
        assert_eq!(
            row.hit_consistency_params().state(),
            SourceValueState::Absent,
            "an absent declaration has no invented child struct"
        );
        assert_eq!(
            row.mineable_instability_params().state(),
            SourceValueState::Null,
            "an authored null pointer remains null"
        );
        assert_eq!(
            row.default_mass().state(),
            SourceValueState::Absent,
            "an absent numeric declaration has no invented zero"
        );
        assert_eq!(
            row.fields().roots().len(),
            1,
            "the original arena contains exactly the actual declaration"
        );
    })?;
    Ok(())
}

#[test]
fn original_string_labels_keep_empty_values_without_assuming_an_enum_class_name() -> TestResult {
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("RefiningProcess").field("refiningSpeed", TestType::String))
        .record(
            TestRecord::new(
                1,
                "RefiningProcess",
                "RefiningProcess.StringSource",
                "libs/foundry/records/refiningprocess/string_source.xml",
            )
            .value("refiningSpeed", TestValue::text("")),
        );
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    with_refining_processes(
        &archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            let row = &snapshot.rows()[0];
            assert_eq!(
                row.refining_speed().value(),
                Some(&""),
                "a present empty String remains present"
            );
            assert_eq!(
                row.refining_quality().state(),
                SourceValueState::Absent,
                "an absent separate label is not an empty String"
            );
            assert!(
                snapshot.coverage().incomplete_fields.is_empty(),
                "the existing String interpretation remains supported"
            );
        },
    )?;
    Ok(())
}
