// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Record, RecordTree, SourceEncryption, StructValue, Value};

use crate::flow_final_fixture::{self as fixture, Case};
use crate::flow_final_original::Original;
use crate::flow_final_original_read;
use crate::flow_final_scalar_source;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

pub(crate) fn check(input: &Fixture, case: Case, origin: Input) -> TestResult<Original> {
    check_origin(input, origin);
    let forge = input.entry.open()?;
    assert_eq!(
        forge.records().len(),
        2,
        "both original physical Records exist"
    );
    let record = forge.record(0).ok_or("actual original template")?;
    check_record(&record)?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "original immutable tree belongs to this exact Record"
    );
    check_tree(&tree, case)?;
    flow_final_scalar_source::check(&tree, case)?;
    let original = flow_final_original_read::capture(&tree)?;
    let expected = if matches!(case, Case::Duplicate) {
        2
    } else {
        1
    };
    assert_eq!(
        original.configurations.len(),
        expected,
        "every original matching declaration, or one genuine absence"
    );
    if !matches!(case, Case::Absent) {
        let first = original
            .configurations
            .first()
            .ok_or("actual first original flow")?;
        assert_eq!(
            first.declaration_index,
            Some(2),
            "original root field ordinal is not an output row index"
        );
        assert_eq!(
            first.nodes.first().ok_or("original root value")?.ordinal,
            3,
            "full-Record DFS ordinal includes preceding owned values"
        );
    }
    Ok(original)
}

pub(crate) fn structure<'tree, 'source>(
    value: &'tree Value<'source>,
) -> TestResult<&'tree StructValue<'source>> {
    match value {
        Value::Struct(value) => Ok(value),
        _ => Err("actual original owned Struct".into()),
    }
}
fn check_origin(input: &Fixture, origin: Input) {
    let expected = match origin {
        Input::Plain => SourceEncryption::Unencrypted,
        Input::Encrypted => SourceEncryption::Encrypted,
        Input::Standalone => SourceEncryption::Unknown,
    };
    assert_eq!(
        input.entry.origin().encryption(),
        expected,
        "actual input encryption before output"
    );
    assert_eq!(
        input.entry.origin().matches_archive(&input.archive),
        !matches!(origin, Input::Standalone),
        "actual physical input lineage"
    );
}

fn check_record(record: &Record<'_>) -> TestResult {
    assert_eq!(record.index(), 0, "actual physical owner position");
    assert_eq!(
        record.guid().to_string(),
        fixture::GUID,
        "actual original owner GUID"
    );
    assert_eq!(
        record.type_name()?,
        "ContractTemplate",
        "real selected family"
    );
    Ok(())
}

fn check_tree(tree: &RecordTree<'_>, case: Case) -> TestResult {
    assert_eq!(
        tree.error_count(),
        usize::from(matches!(case, Case::Holes)),
        "only the intended actual malformed pointer is unreadable"
    );
    assert_eq!(
        tree.field("prefix"),
        Some(&Value::UInt32(42)),
        "real non-flow prefix shifts absolute ordinals"
    );
    let outside = structure(
        tree.field("outsideOwned")
            .ok_or("real outside owned value")?,
    )?;
    assert_eq!(
        outside.field("marker"),
        Some(&Value::Int32(11)),
        "real outside target is retained in the decoded source"
    );
    Ok(())
}
