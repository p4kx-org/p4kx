// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use serde_json::{Value, json};

use crate::health_origin_fixture::{Input, TestResult};
use crate::loot_reference_v3_check::{self as check, Output};
use crate::loot_reference_v3_fixture as fixture;
use crate::loot_v3_graph as graph;
use crate::loot_v3_origin;

#[test]
fn actual_loot_reference_and_weak_receipts_keep_exact_closed_contributor_sets() -> TestResult {
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        check::with_output(input, |output| {
            stored(output)?;
            weak_records(output)?;
            weak_instances(output)?;
            intrinsic_and_nulls(output)
        })?;
    }
    Ok(())
}

fn stored(output: &Output<'_>) -> TestResult {
    for (field, ordinal) in [("first", 2), ("second", 3)] {
        let reference = output.reference(field, "recordReference")?;
        let guid = reference.get("guid").ok_or("authored stored GUID cell")?;
        assert_eq!(
            graph::present(guid)?,
            &json!(fixture::TARGET),
            "exact original stored target GUID"
        );
        output.exact(
            guid,
            &[loot_v3_origin::value_use(
                ordinal,
                "$value",
                SourceUseRole::Payload,
            )],
        )?;
        metadata(output, reference, ordinal, 1)?;
        record_type(
            output,
            reference
                .get("targetType")
                .ok_or("applicable stored Record type")?,
            ordinal,
        )?;
        assert_eq!(
            reference.get("status"),
            Some(&json!("present")),
            "actual admitted target exists"
        );
    }
    Ok(())
}

fn metadata(output: &Output<'_>, reference: &Value, ordinal: u64, target: u32) -> TestResult {
    let path = match target {
        0 => fixture::OWNER_PATH,
        1 => fixture::TARGET_PATH,
        _ => return Err("the actual fixture has only two original target owners".into()),
    };
    for (member, field, role, value) in [
        (
            "targetRecordIndex",
            "recordIndex",
            SourceUseRole::Selector,
            json!(target),
        ),
        (
            "recordPath",
            "fileName",
            SourceUseRole::Conversion,
            json!(path),
        ),
    ] {
        let cell = reference
            .get(member)
            .ok_or("independent original target metadata")?;
        assert_eq!(
            graph::present(cell)?,
            &value,
            "exact original target metadata {member}"
        );
        output.exact(
            cell,
            &[
                check::header(target, field, role),
                loot_v3_origin::value_use(ordinal, "$value", SourceUseRole::Selector),
            ],
        )?;
    }
    Ok(())
}

fn record_type(output: &Output<'_>, cell: &Value, ordinal: u64) -> TestResult {
    assert_eq!(
        graph::present(cell)?,
        &json!("PoolFilterRecord"),
        "actual selected Record type"
    );
    output.exact(
        cell,
        &[
            check::header(1, "sourceType", SourceUseRole::Selector),
            loot_v3_origin::value_use(ordinal, "$value", SourceUseRole::Selector),
        ],
    )
}

fn weak_records(output: &Output<'_>) -> TestResult {
    for (field, ordinal) in [("weakRecord", 4), ("weakRecordAgain", 5)] {
        let reference = output.reference(field, "weakReference")?;
        weak_guid(output, reference, ordinal, 1, fixture::TARGET)?;
        metadata(output, reference, ordinal, 1)?;
        record_type(
            output,
            reference
                .pointer("/target/sourceType")
                .ok_or("applicable weak Record type")?,
            ordinal,
        )?;
        assert_eq!(
            reference.pointer("/target/type"),
            Some(&json!("record")),
            "Record receipt applicability"
        );
        assert!(
            reference.pointer("/target/location").is_none(),
            "Record target has no invented instance location"
        );
    }
    Ok(())
}

fn weak_guid(
    output: &Output<'_>,
    reference: &Value,
    ordinal: u64,
    target: u32,
    guid: &str,
) -> TestResult {
    let cell = reference
        .get("recordGuid")
        .ok_or("actual weak-derived owning GUID")?;
    assert_eq!(
        graph::present(cell)?,
        &json!(guid),
        "weak GUID comes from the actual target owner"
    );
    output.exact(
        cell,
        &[
            check::header(target, "guid", SourceUseRole::Payload),
            loot_v3_origin::value_use(ordinal, "$value", SourceUseRole::Selector),
            loot_v3_origin::value_use(ordinal, "$value", SourceUseRole::Conversion),
        ],
    )
}

fn weak_instances(output: &Output<'_>) -> TestResult {
    for (field, ordinal) in [("weakInstance", 6), ("weakInstanceAgain", 7)] {
        let reference = output.reference(field, "weakReference")?;
        weak_guid(output, reference, ordinal, 0, fixture::OWNER)?;
        metadata(output, reference, ordinal, 0)?;
        assert_eq!(
            reference.pointer("/target/type"),
            Some(&json!("instance")),
            "actual original instance receipt"
        );
        let kind = reference
            .pointer("/target/sourceType")
            .ok_or("original instance type")?;
        assert_eq!(
            graph::present(kind)?,
            &json!("Owned"),
            "never substitute root PoolFilterRecord type"
        );
        output.exact(
            kind,
            &[
                check::traversal("owned.$type", SourceUseRole::Selector),
                loot_v3_origin::value_use(ordinal, "$value", SourceUseRole::Selector),
            ],
        )?;
        let location = reference
            .pointer("/target/location")
            .ok_or("original instance location")?;
        assert_eq!(
            graph::present(location)?,
            &json!("owned"),
            "exact original instance path"
        );
        output.exact(
            location,
            &[
                check::traversal("owned.$fieldName", SourceUseRole::Conversion),
                loot_v3_origin::value_use(ordinal, "$value", SourceUseRole::Selector),
            ],
        )?;
    }
    Ok(())
}

fn intrinsic_and_nulls(output: &Output<'_>) -> TestResult {
    assert_eq!(
        graph::present(output.header(1, "guid")?)?,
        &json!(fixture::TARGET),
        "the intrinsic header retains the same actual selected target GUID"
    );
    for (member, field, role) in [
        ("guid", "guid", SourceUseRole::Payload),
        ("sourceType", "sourceType", SourceUseRole::Selector),
        ("recordIndex", "recordIndex", SourceUseRole::Selector),
        ("recordPath", "fileName", SourceUseRole::Conversion),
    ] {
        output.exact(output.header(1, member)?, &[check::header(1, field, role)])?;
    }
    for (field, ordinal) in [("nullStrong", 8), ("nullWeak", 9), ("nullReference", 10)] {
        let node = output.node(field)?;
        graph::source_null(node);
        output.exact(
            node.get("value").ok_or("actual sourced null")?,
            &[loot_v3_origin::value_use(
                ordinal,
                "$value",
                SourceUseRole::Payload,
            )],
        )?;
    }
    Ok(())
}
