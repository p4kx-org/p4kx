// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::{Query, QueryUnavailable};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::{SourceInput, SourceLocation, SourceUseRole};

use super::FieldStatisticsReport;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn recovered_traversal_failures_keep_original_bytes_and_unavailable_targets() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    for (comparison, label, instance) in [
        ("ptr.ready = true", "pointer", Some(99)),
        ("weak->ready = true", "weakLink", Some(101)),
        (
            "linked->ready = true",
            "reference",
            Some(u32::from(u16::MAX)),
        ),
        ("broken[*] > 0", "arrayHeader", None),
    ] {
        let text = format!("from Station where {comparison} or fallback = true");
        let query = Query::parse(&text)?;
        let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
        assert_eq!(
            report.reduction.statistics().records(),
            1,
            "a terminal error must leave the decisive OR branch available for {comparison}"
        );
        check_attempt(&report, label, instance)?;
        let demanded = Query::parse(&format!("from Station where {comparison}"))?;
        assert!(
            FieldStatisticsReport::query(&forge, SourceInput::Unknown, &demanded).is_err(),
            "the same terminal failure remains an error when demanded: {comparison}"
        );
    }
    Ok(())
}

fn check_attempt(
    report: &FieldStatisticsReport<'_>,
    label: &str,
    target: Option<u32>,
) -> TestResult {
    let use_site = report
        .inventory
        .iter_uses()
        .find(|usage| {
            usage.role() == SourceUseRole::Attempted
                && report.selection_attempts.contains(usage.id())
                && matches!(
                    usage.location(),
                    SourceLocation::Subresource { namespace, field, .. }
                        if namespace == "dataforgeDecodedBytes" && field == label
                )
        })
        .ok_or("the original selector bytes must anchor the attempted traversal")?;
    assert!(
        !report
            .selection_origin
            .source_uses_refs()
            .contains(use_site.id()),
        "the attempted original bytes do not become a completed source use"
    );
    assert_eq!(
        report.unavailable_traversals.len(),
        usize::from(target.is_some()),
        "only a failed instance lookup has a missing instance fact"
    );
    if let Some(instance) = target {
        assert!(
            report.unavailable_traversals.iter().any(|gap| matches!(
                gap,
                QueryUnavailable::Instance { structure: 0, instance: found, .. } if instance == *found
            )),
            "the requested instance is retained without inventing readable target bytes"
        );
    }
    Ok(())
}

fn fixture() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Target").field("ready", TestType::Bool))
        .structure(
            TestStruct::new("Station")
                .field("ptr", TestType::StrongPointer("Target".into()))
                .field("weak", TestType::WeakPointer("Target".into()))
                .field("linked", TestType::ReferenceTo("Target".into()))
                .array("broken", TestType::Int32)
                .field("fallback", TestType::Bool),
        )
        .record(
            TestRecord::new(1, "Station", "Station", "synthetic/station.xml")
                .value("ptr", words(0, 99))
                .value("weak", words(0, 101))
                .value("linked", TestValue::Reference(2))
                .value("broken", words(1, 0))
                .value("fallback", TestValue::Bool(true)),
        )
        .record(
            TestRecord::new(2, "Target", "Unreadable", "synthetic/target.xml")
                .with_instance_index(u16::MAX),
        )
        .build()
}

fn words(first: u32, second: u32) -> TestValue {
    let mut bytes = first.to_le_bytes().to_vec();
    bytes.extend_from_slice(&second.to_le_bytes());
    TestValue::Raw(bytes)
}
