// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use serde_json::{Value, json};

use crate::health_origin_fixture::{Input, TestResult};
use crate::loot_reference_v3_check::{self as check, Output};
use crate::loot_v3_origin::{self as origin, ExpectedUse};

#[test]
fn exact_loot_reference_sets_reject_genuine_id_role_coordinate_and_owner_mutations() -> TestResult {
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        check::with_output(input, |output| {
            stored_metadata(output)?;
            authored_and_intrinsic(output)?;
            weak_operations(output)
        })?;
    }
    Ok(())
}

fn stored_metadata(output: &Output<'_>) -> TestResult {
    let reference = output.reference("first", "recordReference")?;
    let cell = reference
        .get("targetType")
        .ok_or("actual resolved target type")?;
    let header = check::header(1, "sourceType", SourceUseRole::Selector);
    let caller = origin::value_use(2, "$value", SourceUseRole::Selector);
    let expected = [header, caller];
    output.exact(cell, &expected)?;
    remove(output, cell, &expected, &expected[1])?;
    for substitute in [
        origin::value_use(3, "$value", SourceUseRole::Selector),
        origin::value_use(2, "$value", SourceUseRole::Payload),
    ] {
        replace(output, cell, &expected, &expected[1], &substitute)?;
    }
    for substitute in [
        check::header(0, "sourceType", SourceUseRole::Selector),
        check::header(1, "recordIndex", SourceUseRole::Selector),
    ] {
        replace(output, cell, &expected, &expected[0], &substitute)?;
    }
    Ok(())
}

fn authored_and_intrinsic(output: &Output<'_>) -> TestResult {
    let intrinsic = output.header(1, "guid")?;
    let header = [check::header(1, "guid", SourceUseRole::Payload)];
    let caller = origin::value_use(2, "$value", SourceUseRole::Selector);
    output.exact(intrinsic, &header)?;
    add(output, intrinsic, &header, &caller)?;
    let reference = output.reference("first", "recordReference")?;
    let authored = reference.get("guid").ok_or("actual authored GUID")?;
    let token = [origin::value_use(2, "$value", SourceUseRole::Payload)];
    output.exact(authored, &token)?;
    add(output, authored, &token, &header[0])?;
    add(output, authored, &token, &caller)?;
    replace(output, authored, &token, &token[0], &header[0])
}

fn weak_operations(output: &Output<'_>) -> TestResult {
    let reference = output.reference("weakRecord", "weakReference")?;
    let cell = reference
        .get("recordGuid")
        .ok_or("actual weak-derived GUID")?;
    let expected = [
        check::header(1, "guid", SourceUseRole::Payload),
        origin::value_use(4, "$value", SourceUseRole::Selector),
        origin::value_use(4, "$value", SourceUseRole::Conversion),
    ];
    output.exact(cell, &expected)?;
    remove(output, cell, &expected, &expected[1])?;
    remove(output, cell, &expected, &expected[2])?;
    replace(
        output,
        cell,
        &expected,
        &expected[1],
        &origin::value_use(5, "$value", SourceUseRole::Selector),
    )?;
    replace(
        output,
        cell,
        &expected,
        &expected[2],
        &origin::value_use(5, "$value", SourceUseRole::Conversion),
    )?;
    replace(
        output,
        cell,
        &expected,
        &expected[0],
        &origin::value_use(2, "$value", SourceUseRole::Payload),
    )
}

fn remove(
    output: &Output<'_>,
    cell: &Value,
    expected: &[ExpectedUse],
    removed: &ExpectedUse,
) -> TestResult {
    let id = output.id(removed)?;
    let mut references = refs(cell)?;
    let before = references.len();
    references.retain(|reference| reference.as_str() != Some(id));
    assert_eq!(
        references.len() + 1,
        before,
        "remove exactly one genuine original contribution"
    );
    let changed = changed(cell, references)?;
    output.rejected(&changed, expected);
    Ok(())
}

fn replace(
    output: &Output<'_>,
    cell: &Value,
    expected: &[ExpectedUse],
    original: &ExpectedUse,
    replacement: &ExpectedUse,
) -> TestResult {
    let original = output.id(original)?;
    let replacement = output.id(replacement)?;
    assert_ne!(original, replacement, "two distinct genuinely emitted IDs");
    let mut references = refs(cell)?;
    assert!(
        !references
            .iter()
            .any(|reference| reference.as_str() == Some(replacement)),
        "a duplicate-ID guard must not mask the intended contribution failure"
    );
    let positions: Vec<_> = references
        .iter()
        .enumerate()
        .filter_map(|(position, reference)| {
            (reference.as_str() == Some(original)).then_some(position)
        })
        .collect();
    let [position] = positions.as_slice() else {
        return Err("exactly one genuine original contribution to substitute".into());
    };
    *references
        .get_mut(*position)
        .ok_or("actual original ID position")? = json!(replacement);
    let changed = changed(cell, references)?;
    assert_eq!(
        refs(&changed)?.len(),
        refs(cell)?.len(),
        "same-cardinality genuine-ID substitution"
    );
    output.rejected(&changed, expected);
    Ok(())
}

fn add(
    output: &Output<'_>,
    cell: &Value,
    expected: &[ExpectedUse],
    added: &ExpectedUse,
) -> TestResult {
    let id = output.id(added)?;
    let mut references = refs(cell)?;
    assert!(
        !references
            .iter()
            .any(|reference| reference.as_str() == Some(id)),
        "adding a genuine forbidden taint is not a duplicate-ID control"
    );
    references.push(json!(id));
    let changed = changed(cell, references)?;
    output.rejected(&changed, expected);
    Ok(())
}

fn refs(cell: &Value) -> TestResult<Vec<Value>> {
    cell.get("sourceUsesRefs")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| "actual immediate source references".into())
}

fn changed(original: &Value, references: Vec<Value>) -> TestResult<Value> {
    let mut changed = original.clone();
    changed
        .as_object_mut()
        .ok_or("actual sourced cell object")?
        .insert("sourceUsesRefs".to_owned(), Value::Array(references));
    for member in [
        "state",
        "value",
        "containsEncryptedSource",
        "sourceSetComplete",
    ] {
        assert_eq!(
            changed.get(member),
            original.get(member),
            "only contributor refs change, original {member} remains byte-value equal"
        );
    }
    Ok(changed)
}
