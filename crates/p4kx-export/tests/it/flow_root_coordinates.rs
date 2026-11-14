// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::flow_owner_graph::Owner;
use crate::flow_root_fixture::Case;
use crate::flow_root_positions::Original;
use crate::health_origin_fixture::TestResult;

pub(super) fn check(
    owner: &Owner<'_>,
    fields: &[Value],
    original: &Original,
    case: Case,
) -> TestResult {
    for (reference, instance, field_index, name) in [
        (
            fields.first().ok_or("selected id declaration")?,
            &original.target,
            0,
            "id",
        ),
        (
            fields.get(1).ok_or("selected variable declaration")?,
            &original.property_instance,
            u64::from(matches!(case, Case::InsideFlow)),
            "missionVariableName",
        ),
    ] {
        let expected = (Some(original.ordinal(instance)?), field_index);
        assert_eq!(
            owner.position(reference)?,
            expected,
            "original selected instance and field ordinals survive a non-emitted parent"
        );
        let declaration = owner.field(reference)?;
        assert_eq!(
            declaration.pointer("/name/value"),
            Some(&json!(name)),
            "actual selected declaration name"
        );
        if reference.get("type") == Some(&json!("root")) {
            cut_root_controls(declaration, expected, original)?;
        }
    }
    Ok(())
}

fn cut_root_controls(
    declaration: &Value,
    expected: (Option<u64>, u64),
    original: &Original,
) -> TestResult {
    root_position(declaration, expected)?;
    let mut changed = declaration.clone();
    *changed
        .pointer_mut("/sourcePosition/parentOrdinal")
        .ok_or("retained original parent coordinate")? =
        json!(original.ordinal(&original.flow)?.to_string());
    assert!(
        root_position(&changed, expected).is_err(),
        "another genuine same-owner ordinal cannot replace the selected original parent"
    );
    let mut changed = declaration.clone();
    *changed
        .pointer_mut("/sourcePosition/fieldIndex")
        .ok_or("retained original field coordinate")? = json!((expected.1 + 1).to_string());
    assert!(
        root_position(&changed, expected).is_err(),
        "another field ordinal cannot replace the actual original declaration position"
    );
    let mut changed = declaration.clone();
    *changed
        .pointer_mut("/sourcePosition/parentOrdinal")
        .ok_or("retained original parent coordinate")? = Value::Null;
    assert!(
        root_position(&changed, expected).is_err(),
        "a cut nested root cannot claim the genuine Record-root control"
    );
    Ok(())
}

fn root_position(field: &Value, expected: (Option<u64>, u64)) -> TestResult {
    let expected = json!({
        "parentOrdinal": expected.0.map(|ordinal| ordinal.to_string()),
        "fieldIndex": expected.1.to_string(),
    });
    if field.get("sourcePosition") != Some(&expected) {
        return Err("selected root must retain exact genuine parent and field coordinates".into());
    }
    Ok(())
}
