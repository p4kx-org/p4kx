// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Mutex;

use p4kx_dataforge::FieldStatisticsUse;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::FieldStatisticsSourceUses;
use crate::{DataForge, FieldStatistics, FieldStatisticsSourceScope};
use crate::{InputScope, SourceInput, SourceLocation, SourceUse, SourceUseRole, SourceUses};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn binds_only_actual_parent_and_value_sites_in_one_source_inventory() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let uses = SourceUses::with_compact_use_refs();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    let result = Mutex::new(None);
    FieldStatistics::of_records_observed(&forge, &[0], |event| {
        if matches!(event.usage(), FieldStatisticsUse::Counted { .. })
            && event.leaf_to_root().next() == Some("childValue")
        {
            assert_eq!(
                event.enclosing_route().count(),
                2,
                "original array then instance"
            );
            *result.lock().expect("the test owns the receipt") =
                Some(FieldStatisticsSourceUses::bind(&scope, &event));
        }
    })?;
    let result = result
        .into_inner()?
        .ok_or("the measured scalar must exist")??;
    check_bound_sites(&result)
}

fn check_bound_sites(result: &FieldStatisticsSourceUses) -> TestResult {
    let selectors = result.selectors();
    assert!(
        selectors
            .iter()
            .any(|site| has_site(site, "dataforgeDecodedBytes", "arrayHeader")),
        "the actual array header selects the embedded instance",
    );
    assert!(
        selectors
            .iter()
            .any(|site| has_site(site, "dataforgeArrayElement", "selection")),
        "the author-selected array element survives into the counted value",
    );
    assert!(
        selectors
            .iter()
            .all(|site| site.role() == SourceUseRole::Selector),
        "selected route uses remain selector evidence",
    );
    let [operand] = result.operands() else {
        return Err("the counted scalar must have one original value operand".into());
    };
    assert!(
        has_site(operand, "dataforgeDecodedBytes", "slot"),
        "only scalar bytes contribute the value"
    );
    assert_eq!(
        operand.role(),
        SourceUseRole::Payload,
        "numeric field payload role"
    );
    assert_eq!(
        result.attempts().len(),
        0,
        "the accepted route did not fail any original reads"
    );
    assert_eq!(
        result.source_scope(),
        FieldStatisticsSourceScope::SelectionAndPathsUnbound,
        "one exact observed association does not prove aggregate membership",
    );
    Ok(())
}

fn fixture() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Child").field("childValue", TestType::Int32))
        .structure(TestStruct::new("Owner").array("children", TestType::Class("Child".to_owned())))
        .record(
            TestRecord::new(1, "Owner", "One", "libs/fixture.xml").value(
                "children",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new("Child").value("childValue", TestValue::Int32(41)),
                )]),
            ),
        )
        .build()
}

fn has_site(site: &SourceUse, expected_namespace: &str, expected_field: &str) -> bool {
    matches!(
        site.description().location(),
        SourceLocation::Subresource { namespace, field, .. }
            if namespace == expected_namespace && field == expected_field
    )
}
