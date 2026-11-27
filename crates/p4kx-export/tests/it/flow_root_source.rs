// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Record, ReferenceOrigin, SourceEncryption, Step as LocationStep, StructValue, Value,
};

use crate::flow_root_fixture::{self as fixture, Case};
use crate::flow_root_positions::{self as positions, Original, Step};
use crate::health_origin_fixture::{Fixture, Input, TestResult};

pub(crate) fn check(input: &Fixture, case: Case, origin: Input) -> TestResult<Original> {
    let expected = match origin {
        Input::Plain => SourceEncryption::Unencrypted,
        Input::Encrypted => SourceEncryption::Encrypted,
        Input::Standalone => SourceEncryption::Unknown,
    };
    assert_eq!(
        input.entry.origin().encryption(),
        expected,
        "genuine original input encryption"
    );
    let forge = input.entry.open()?;
    assert_eq!(
        forge.records().len(),
        1,
        "one actual same-owner source Record"
    );
    let record = forge.record(0).ok_or("actual source template")?;
    assert_eq!(
        record.guid().to_string(),
        fixture::GUID,
        "actual source GUID"
    );
    let tree = record.tree()?;
    assert!(tree.is_from(&record), "actual original pairing");
    assert_eq!(
        tree.error_count(),
        0,
        "inside/overlap and nonfinite source facts are fully readable"
    );
    let original = positions::snapshot(tree.root(), case);
    values(tree.root(), &original, case)?;
    pointers(tree.root(), &original, &record)?;
    let union = original.union()?;
    assert!(
        union
            .windows(2)
            .all(|pair| matches!(pair, [left, right] if left < right)),
        "distinct genuine occurrence union is in original DFS order"
    );
    if matches!(case, Case::InsideFlow) {
        assert_eq!(
            union.len(),
            original.paths.len(),
            "selected fields already lie in the complete Flow source subtree"
        );
    } else {
        assert!(
            original.variable.starts_with(&original.id),
            "selected property field is physically inside the selected id subtree"
        );
        assert!(
            union.len() < original.paths.len(),
            "unselected owner facts are not a whole-Record dump"
        );
    }
    Ok(original)
}

fn values(root: &StructValue<'_>, original: &Original, case: Case) -> TestResult {
    let id = positions::at(root, &original.id)?;
    let variable = positions::at(root, &original.variable)?;
    assert_eq!(
        variable,
        &Value::String(fixture::VARIABLE),
        "actual selected property value is preserved exactly"
    );
    if matches!(case, Case::InsideFlow) {
        let Value::Guid(value) = id else {
            return Err("actual inside original GUID".into());
        };
        assert_eq!(
            value.to_string(),
            fixture::ID,
            "actual inside id differs from owner header"
        );
        assert_ne!(
            value.to_string(),
            fixture::GUID,
            "no owning-header substitute"
        );
    } else {
        let Value::Struct(value) = id else {
            return Err("actual wrong-shaped id subtree".into());
        };
        assert_eq!(
            value.type_name(),
            "NestedTarget",
            "readable wrong-shaped source remains a real Struct"
        );
        assert_eq!(
            value.field("flag"),
            Some(&Value::Bool(false)),
            "extra original false field"
        );
        let Some(Value::Double(number)) = value.field("nonfinite") else {
            return Err("actual selected-root Double".into());
        };
        assert_eq!(
            number.to_bits(),
            fixture::NAN,
            "exact selected nonfinite outside missionFlow"
        );
    }
    Ok(())
}

fn pointers(root: &StructValue<'_>, original: &Original, record: &Record<'_>) -> TestResult {
    for (caller, target, expected_type) in [
        (&original.objective_caller, &original.target, "FlowTarget"),
        (
            &original.property_caller,
            &original.property_instance,
            if original.property_instance == original.target {
                "FlowTarget"
            } else {
                "NestedTarget"
            },
        ),
    ] {
        let Value::Reference(reference) = positions::at(root, caller)? else {
            return Err("actual original resolved weak user".into());
        };
        let receipt = reference.original_target();
        assert!(
            receipt.shares_source_with(record),
            "authentic same original Forge receipt"
        );
        assert_eq!(
            receipt.record_index(),
            record.index(),
            "actual same-owner target"
        );
        let positions: Vec<_> = target
            .iter()
            .map(|step| match step {
                Step::Field(index) | Step::Element(index) => u64::try_from(*index),
            })
            .collect::<Result<_, _>>()?;
        assert_eq!(
            receipt.instance_positions(),
            Some(positions.as_slice()),
            "authentic typed original target tuple"
        );
        assert_eq!(
            reference.type_name(),
            expected_type,
            "actual selected instance type"
        );
        typed_location(receipt, target.len())?;
        assert!(
            receipt.record_type().is_none(),
            "instance metadata does not become root header metadata"
        );
    }
    Ok(())
}

fn typed_location(receipt: ReferenceOrigin<'_, '_>, expected_len: usize) -> TestResult {
    let location = receipt
        .instance_location()
        .ok_or("actual instance location")?;
    assert_eq!(
        location.steps().len(),
        expected_len,
        "actual typed position length"
    );
    assert!(
        location
            .steps()
            .iter()
            .all(|step| matches!(step, LocationStep::Field(_))),
        "these target receipts contain actual Field edges, not array edges with equal numbers"
    );
    Ok(())
}
