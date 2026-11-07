// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Unsupported or unreadable scoped filters fail their Record instead of becoming null.

use p4kx_testkit::{TestInstance, TestValue};

use crate::scoped_fixture::{self, TestResult, object, property, scoped};

#[test]
fn an_unknown_concrete_variant_in_each_family_fails_both_owning_records() -> TestResult {
    for (field, source_type) in [
        ("factionFilter", "LocationCondition_UnknownFaction"),
        ("locationClassFilter", "LocationCondition_UnknownClass"),
        ("locationSizeFilter", "LocationCondition_UnknownSize"),
        (
            "locationFunctionFilter",
            "LocationCondition_UnknownFunction",
        ),
        ("otherLocationFilter", "LocationCondition_UnknownOther"),
        (
            "relativeLocationFilter",
            "LocationCondition_UnknownRelative",
        ),
    ] {
        let report = scoped_fixture::diagnostic(vec![property(
            "Scoped_BP",
            scoped([(field, object(TestInstance::new(source_type)))]),
        )])?;
        assert_eq!(report.failures.len(), 2, "{field}: {:?}", report.failures);
        assert!(
            report
                .failures
                .iter()
                .all(|error| error.contains("unknown struct") && error.contains(source_type))
        );
        assert_eq!(
            report.document["contractTemplates"]
                .as_array()
                .ok_or("templates")?
                .len(),
            0,
            "an unreadable source Record is not emitted"
        );
        assert_eq!(
            report.document["missionBrokerEntries"]
                .as_array()
                .ok_or("brokers")?
                .len(),
            0,
            "an unreadable source Record is not emitted"
        );
    }
    Ok(())
}

#[test]
fn an_unreadable_populated_filter_is_not_exported_as_an_absent_pointer() -> TestResult {
    let bad: Vec<u8> = (u32::MAX - 1)
        .to_le_bytes()
        .into_iter()
        .chain(3_u32.to_le_bytes())
        .collect();
    let report = scoped_fixture::diagnostic(vec![property(
        "Scoped_BP",
        scoped([("locationClassFilter", TestValue::Raw(bad))]),
    )])?;
    assert_eq!(report.failures.len(), 2, "{:?}", report.failures);
    assert!(
        report
            .failures
            .iter()
            .all(|error| error.contains("locationClassFilter") && error.contains("unreadable"))
    );
    assert_eq!(
        report.document["contractTemplates"]
            .as_array()
            .ok_or("templates")?
            .len(),
        0,
        "an unreadable source Record is not emitted"
    );
    Ok(())
}

#[test]
fn an_unreadable_enum_is_distinct_from_a_preserved_unknown_enum_name() -> TestResult {
    let report = scoped_fixture::diagnostic(vec![property(
        "Scoped_BP",
        scoped([(
            "factionFilter",
            object(
                TestInstance::new("LocationCondition_RequiredFactionHostility")
                    .value("hostility", TestValue::Raw(u32::MAX.to_le_bytes().to_vec())),
            ),
        )]),
    )])?;
    assert_eq!(report.failures.len(), 2, "{:?}", report.failures);
    assert!(
        report
            .failures
            .iter()
            .all(|error| error.contains("hostility") && error.contains("unreadable"))
    );
    Ok(())
}
