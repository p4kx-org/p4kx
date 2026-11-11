// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Record, RecordTree, SourceUse, SourceValue, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::{Consulted, Decisions, Fields};
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};

fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("TextOwner")
                .array("first", TestType::UInt32)
                .array("second", TestType::UInt32),
        )
        .record(
            TestRecord::new(
                0x7155,
                "TextOwner",
                "TextOwner.Original",
                "synthetic/TextOwner.xml",
            )
            .value(
                "first",
                TestValue::Array(vec![
                    TestValue::UInt32(2),
                    TestValue::UInt32(3),
                    TestValue::UInt32(5),
                    TestValue::UInt32(7),
                ]),
            )
            .value(
                "second",
                TestValue::Array(vec![TestValue::UInt32(2), TestValue::UInt32(3)]),
            ),
        )
        .build()
}

#[test]
fn repeated_prefixes_keep_the_exact_output_and_visit_only_each_new_suffix() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("original text owner")?;
        let tree = record.tree()?;
        source(&record, &tree);
        let scope = input.record_scope(&record, &tree)?;
        let original = OriginalFlowFields::read(&scope, None)?;
        let fields = Fields::new(&original, input.origins())?;
        let first = decisions(&fields, "first")?;
        let second = decisions(&fields, "second")?;
        let (mut consulted, repeated) = accumulate(&first, &second, input.origins())?;
        equal_output(&consulted, &repeated, input.origins())?;
        reject_end(&mut consulted, &first, input.origins());
        fresh_language(&first, input.origins())?;
        Ok(())
    })
}

fn source<'source>(record: &Record<'source>, tree: &RecordTree<'source>) {
    assert!(
        tree.is_from(record),
        "the source tree belongs to the actual original Record"
    );
    assert_eq!(
        tree.error_count(),
        0,
        "all controlled source fields are readable"
    );
    assert!(
        matches!(tree.field("first"), Some(Value::Array(values))
        if values.as_ref() == [Value::UInt32(2), Value::UInt32(3), Value::UInt32(5), Value::UInt32(7)]),
        "the original ordered inputs are independently readable"
    );
    assert!(
        matches!(tree.field("second"), Some(Value::Array(values))
        if values.as_ref() == [Value::UInt32(2), Value::UInt32(3)]),
        "equal source bytes at the other site have their own physical occurrence"
    );
}

fn accumulate<'read>(
    first: &'read Decisions,
    second: &'read Decisions,
    origins: &CatalogOrigins,
) -> TestResult<(Consulted<'read>, Vec<SourceUse>)> {
    let mut consulted = Consulted::new(&[])?;
    let mut repeated = Vec::new();
    let mut largest = 0;
    for end in [2, 4, 1, 5, 3, 5] {
        largest = largest.max(end);
        first.retain_range(0, end, origins, &mut repeated)?;
        consulted.prefix(first, end, origins)?;
        assert_eq!(
            consulted.uses().len(),
            largest,
            "only new original selector suffixes are materialized"
        );
    }
    assert_eq!(
        consulted.uses().len(),
        first.len(),
        "the first complete prefix retains every actual input once"
    );
    second.retain(origins, &mut repeated)?;
    consulted.all(second, origins)?;
    assert_eq!(
        consulted.uses().len(),
        first.len() + second.len(),
        "independent real sites do not share their retained-prefix cursor"
    );
    Ok((consulted, repeated))
}

fn equal_output(
    consulted: &Consulted<'_>,
    repeated: &[SourceUse],
    origins: &CatalogOrigins,
) -> TestResult {
    let expected = origins
        .uses
        .value(SourceValue::Present("same display"), repeated, true);
    let actual = origins
        .uses
        .value(SourceValue::Present("same display"), consulted.uses(), true);
    assert_eq!(
        serde_json::to_value(&actual)?,
        serde_json::to_value(&expected)?,
        "semantic output, direct marker, completeness and exact sorted source refs are identical"
    );
    Ok(())
}

fn reject_end<'read>(
    consulted: &mut Consulted<'read>,
    first: &'read Decisions,
    origins: &CatalogOrigins,
) {
    let before = consulted.uses().len();
    assert!(
        consulted.prefix(first, first.len() + 1, origins).is_err(),
        "a prefix beyond the actual source is an error"
    );
    assert_eq!(
        consulted.uses().len(),
        before,
        "the rejected prefix adds no partial evidence"
    );
}

fn fresh_language(first: &Decisions, origins: &CatalogOrigins) -> TestResult {
    let mut language = Consulted::new(&[])?;
    language.all(first, origins)?;
    assert_eq!(
        language.uses().len(),
        first.len(),
        "a distinct language render starts with its own consulted inputs"
    );
    Ok(())
}
fn decisions(fields: &Fields<'_, '_, '_, '_>, name: &str) -> TestResult<Decisions> {
    let mut decisions = fields.decisions();
    let value = fields
        .field(None, name, &mut decisions)?
        .ok_or("actual source array")?;
    for element in fields.items(value)? {
        fields.record(element, OriginalValuePart::Value, &mut decisions)?;
    }
    Ok(decisions)
}
