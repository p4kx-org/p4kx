// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, RecordTree, Reference, StructValue, Value, ValueError};

use super::fixture;
use crate::CatalogError;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueEdge};
use crate::template_reference_index::{TargetSelectionKind, TemplateReferences};

#[test]
fn genuine_forward_backward_and_array_receipts_select_original_typed_positions() -> TestResult {
    for values in [fixture::Values::Distinct, fixture::Values::Equal] {
        test_support::with_input(&fixture::database(values), |forge, input| {
            let record = forge.record(0).ok_or("actual original Record")?;
            let tree = record.tree()?;
            source(&tree, values)?;
            let scope = input.record_scope(&record, &tree)?;
            let fields = OriginalFlowFields::read(&scope, None)?;
            selections(&fields, tree.root(), values)?;
            identity(&fields, values)
        })?;
    }
    Ok(())
}

fn source(tree: &RecordTree<'_>, values: fixture::Values) -> TestResult {
    assert_eq!(
        tree.error_count(),
        1,
        "one original invalid pointer array slot"
    );
    let root = tree.root();
    assert_eq!(
        root.fields()
            .iter()
            .filter(|field| field.name() == "target")
            .count(),
        2,
        "two actual equal-spelling declarations"
    );
    positions(reference(root, "forward")?, &[4])?;
    positions(reference(root, "backward")?, &[0])?;
    positions(reference(root, "arrayTarget")?, &[2, 3])?;
    original_target(
        root.fields().first().ok_or("real early field")?.value(),
        fixture::BEFORE,
        "before-original",
    )?;
    let (after, variable) = after(values);
    original_target(
        root.fields().get(4).ok_or("real late field")?.value(),
        after,
        variable,
    )?;
    array_source(root, after, variable)
}

fn array_source(root: &StructValue<'_>, guid: &str, variable: &str) -> TestResult {
    let Some(Value::Array(items)) = root.field("targets") else {
        return Err("actual original target array".into());
    };
    assert_eq!(items.len(), 4, "four actual canonical array occurrences");
    assert!(
        matches!(items.get(1), Some(Value::Null)),
        "actual original middle null"
    );
    assert!(
        matches!(
            items.get(2),
            Some(Value::Error(ValueError::InvalidPointer {
                struct_index: 77,
                instance_index: 99
            }))
        ),
        "the exact two original invalid pointer words"
    );
    original_target(items.get(3).ok_or("actual array target")?, guid, variable)
}
fn after(values: fixture::Values) -> (&'static str, &'static str) {
    if matches!(values, fixture::Values::Equal) {
        (fixture::BEFORE, "before-original")
    } else {
        (fixture::AFTER, " after-original ")
    }
}

fn original_target(value: &Value<'_>, guid: &str, variable: &str) -> TestResult {
    let Value::Struct(target) = value else {
        return Err("actual original target struct".into());
    };
    let Some(Value::Guid(id)) = target.field("id") else {
        return Err("actual original target GUID".into());
    };
    assert_eq!(
        id.to_string(),
        guid,
        "the original target field, independently decoded"
    );
    let Some(Value::String(text)) = target.field("missionVariableName") else {
        return Err("actual original target string".into());
    };
    assert_eq!(
        *text, variable,
        "the original target string, independently decoded"
    );
    Ok(())
}
fn selections<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    root: &StructValue<'source>,
    values: fixture::Values,
) -> TestResult {
    let (after, variable) = after(values);
    assert_eq!(
        objective(fields, root, "forward")?
            .ok_or("late source id")?
            .to_string(),
        after,
        "genuine forward target rather than first matching text path"
    );
    assert_eq!(
        objective(fields, root, "backward")?
            .ok_or("early source id")?
            .to_string(),
        fixture::BEFORE,
        "genuine backward target"
    );
    assert_eq!(
        objective(fields, root, "arrayTarget")?
            .ok_or("array source id")?
            .to_string(),
        after,
        "actual array kind and original index"
    );
    assert_eq!(
        selected(fields, root, "forward", "missionVariableName")?,
        Some(&Value::String(variable)),
        "the target field's exact original whitespace"
    );
    Ok(())
}

fn identity(fields: &OriginalFlowFields<'_, '_, '_>, values: fixture::Values) -> TestResult {
    let early = fields.child(None, OriginalValueEdge::Field { ordinal: 0 })?;
    let late = fields.child(None, OriginalValueEdge::Field { ordinal: 4 })?;
    assert_ne!(
        early.ordinal(),
        late.ordinal(),
        "equal values are still distinct source occurrences"
    );
    assert!(
        !std::ptr::eq(early.value(), late.value()),
        "different actual source slots"
    );
    if matches!(values, fixture::Values::Equal) {
        assert_eq!(
            early.value(),
            late.value(),
            "actual equal-content negative identity control"
        );
    }
    Ok(())
}

fn reference<'read, 'source>(
    root: &'read StructValue<'source>,
    name: &str,
) -> TestResult<&'read Reference<'source>> {
    let Some(Value::Reference(reference)) = root.field(name) else {
        return Err("actual original resolved weak reference".into());
    };
    Ok(reference)
}

fn positions(reference: &Reference<'_>, expected: &[u64]) -> TestResult {
    let original = reference.original_target();
    assert_eq!(
        original.instance_positions(),
        Some(expected),
        "genuine decoder position tuple"
    );
    assert_eq!(
        original
            .instance_location()
            .ok_or("genuine original location")?
            .steps()
            .len(),
        expected.len(),
        "the actual typed location and tuple agree"
    );
    Ok(())
}

#[test]
fn wrong_in_range_target_and_caller_entries_cannot_alias_genuine_facts() -> TestResult {
    test_support::with_input(
        &fixture::database(fixture::Values::Distinct),
        |forge, input| {
            let record = forge.record(0).ok_or("actual original Record")?;
            let tree = record.tree()?;
            source(&tree, fixture::Values::Distinct)?;
            let scope = input.record_scope(&record, &tree)?;
            let mut fields = OriginalFlowFields::read(&scope, None)?;
            assert_eq!(
                objective(&fields, tree.root(), "forward")?
                    .ok_or("positive late")?
                    .to_string(),
                fixture::AFTER,
                "real unmutated typed position positive"
            );
            wrong_target(&mut fields)?;
            super::conflicting(&objective(&fields, tree.root(), "forward"));
            let mut fields = OriginalFlowFields::read(&scope, None)?;
            wrong_caller(&mut fields, tree.root())?;
            super::conflicting(&objective(&fields, tree.root(), "forward"));
            Ok(())
        },
    )
}

fn wrong_target(fields: &mut OriginalFlowFields<'_, '_, '_>) -> TestResult {
    let early = *fields
        .edges
        .get(&(None, OriginalValueEdge::Field { ordinal: 0 }))
        .ok_or("actual early physical index")?;
    fields
        .edges
        .insert((None, OriginalValueEdge::Field { ordinal: 4 }), early);
    Ok(())
}

fn wrong_caller<'source>(
    fields: &mut OriginalFlowFields<'_, '_, 'source>,
    root: &StructValue<'source>,
) -> TestResult {
    let forward = root.field("forward").ok_or("real forward caller")?;
    let backward = root.field("backward").ok_or("real backward caller")?;
    let other = *fields
        .values
        .get(&std::ptr::from_ref(backward).cast::<()>())
        .ok_or("real other caller")?;
    fields
        .values
        .insert(std::ptr::from_ref(forward).cast::<()>(), other);
    Ok(())
}

fn objective<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    parent: &StructValue<'source>,
    name: &str,
) -> Result<Option<Guid>, CatalogError> {
    Ok(match selected(fields, parent, name, "id")? {
        Some(Value::Guid(id)) => Some(*id),
        _ => None,
    })
}

fn selected<'read, 'source>(
    fields: &OriginalFlowFields<'_, 'read, 'source>,
    parent: &StructValue<'source>,
    name: &str,
    target: &str,
) -> Result<Option<&'read Value<'source>>, CatalogError> {
    let Some(value @ Value::Reference(_)) = parent.field(name) else {
        return Ok(None);
    };
    let caller = fields.fact(value)?;
    let mut references = TemplateReferences::new(fields);
    let actual = references.select_fact(caller, target)?;
    Ok(match actual.kind() {
        TargetSelectionKind::Instance {
            field: Some(field), ..
        } => Some(field.value()),
        _ => None,
    })
}
