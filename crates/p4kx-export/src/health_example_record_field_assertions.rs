// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourceUseRole, SourceUses, SourcedValue};

use super::LOCATION;
use crate::HealthText;
use crate::build::original_input::test_support::TestResult;
use crate::catalog_read::original::health_example_body::{ExampleBody, ExampleBodyReceipt};

pub(super) fn exact_body(body: ExampleBodyReceipt, sources: &SourceUses) -> TestResult {
    let ExampleBody::Captured {
        frame,
        field,
        source_type,
        target_guid,
    } = body.into_body()
    else {
        return Err("actual captured collision body".into());
    };
    let field = field.finish(&frame)?.into_cell(sources);
    let source_type = source_type.finish(&frame)?.into_cell(sources);
    let target_guid = target_guid.finish(&frame)?.into_cell(sources);
    assert_eq!(
        field.value().map(HealthText::as_str),
        Some(LOCATION),
        "the complete actual dynamic path"
    );
    exact_uses(&field, sources, field_uses())?;
    assert_eq!(
        source_type.value().map(HealthText::as_str),
        Some("Owner"),
        "collision identity describes its actual Record header rather than Params"
    );
    exact_uses(
        &source_type,
        sources,
        vec![(
            "dataforgeRecordHeader",
            vec![0],
            "sourceType",
            SourceUseRole::Selector,
        )],
    )?;
    assert!(
        target_guid.value().is_none() && target_guid.origin().source_uses_refs().is_empty(),
        "this operation has no target GUID datum"
    );
    Ok(())
}

fn field_uses() -> Vec<Use> {
    vec![
        (
            "dataforgeValue",
            vec![0, 1],
            "$fieldName",
            SourceUseRole::Conversion,
        ),
        (
            "dataforgeValue",
            vec![0, 2],
            "$fieldName",
            SourceUseRole::Conversion,
        ),
        (
            "dataforgeValue",
            vec![0, 2],
            "$value",
            SourceUseRole::Selector,
        ),
    ]
}

type Use = (&'static str, Vec<u64>, &'static str, SourceUseRole);

fn exact_uses(
    cell: &SourcedValue<HealthText>,
    sources: &SourceUses,
    expected: Vec<Use>,
) -> TestResult {
    assert!(
        cell.origin().source_set_complete(),
        "every actual contributor is retained"
    );
    assert_eq!(
        cell.origin().contains_encrypted_source(),
        Some(false),
        "actual plain Archive source"
    );
    let inventory = sources.freeze()?;
    let mut actual = Vec::new();
    for id in cell.origin().source_uses_refs() {
        let used = inventory
            .uses()
            .iter()
            .find(|used| used.id() == id)
            .ok_or("actual contributing use")?;
        actual.push((used.location().clone(), used.role()));
    }
    let expected: Vec<_> = expected
        .into_iter()
        .map(|(namespace, indices, field, role)| {
            (
                SourceLocation::Subresource {
                    namespace: namespace.to_owned(),
                    indices,
                    field: field.to_owned(),
                },
                role,
            )
        })
        .collect();
    assert_eq!(
        actual.len(),
        expected.len(),
        "the complete contributor cardinality is exact"
    );
    for expected in expected {
        assert_eq!(
            actual.iter().filter(|actual| **actual == expected).count(),
            1,
            "each exact original position and role occurs once in the closed contributor set"
        );
    }
    Ok(())
}
