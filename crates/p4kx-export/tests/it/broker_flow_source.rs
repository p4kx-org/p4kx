// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Field, Value};

use crate::broker_flow_fixture::GUID;
use crate::health_origin_fixture::{Fixture, TestResult};
use crate::template_flow_depth_fixture::{Branch, Tail};

pub(crate) fn check(input: &Fixture, branch: Branch, count: usize, tail: Tail) -> TestResult {
    crate::template_flow_depth_source::check_owner(
        input,
        branch,
        count,
        tail,
        "MissionBrokerEntry",
        GUID,
    )?;
    let forge = input.entry.open()?;
    let record = forge.record(0).ok_or("actual original physical Broker")?;
    assert!(
        forge.record(1).is_none(),
        "one physical original owner, independent of declared unused types"
    );
    let tree = record.tree()?;
    let width = if matches!(branch, Branch::Not) { 1 } else { 2 };
    let extra = usize::from(matches!(tail, Tail::Missing | Tail::Empty));
    let expected = count
        .checked_mul(width)
        .and_then(|value| value.checked_add(4 + extra))
        .ok_or("exact original occurrence count")?;
    let mut pending: Vec<_> = tree.fields().iter().rev().map(Field::value).collect();
    let mut actual = 0_usize;
    while let Some(value) = pending.pop() {
        actual = actual.checked_add(1).ok_or("actual original DFS count")?;
        match value {
            Value::Struct(source) => pending.extend(source.fields().iter().rev().map(Field::value)),
            Value::Array(elements) => pending.extend(elements.iter().rev()),
            _ => {}
        }
    }
    assert_eq!(
        actual, expected,
        "all actual source occurrences are proven before the producer"
    );
    Ok(())
}
