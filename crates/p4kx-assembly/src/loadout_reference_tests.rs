// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::ptr;
use std::sync::Arc;

use p4kx_dataforge::{DataForge, Record, RecordTree, StructValue, Value};

use super::LoadoutReference;
use crate::loadout_path::PathRead;
use crate::spec::{EntitySpec, LoadoutSpec, TargetSpec};
use crate::{
    Entities, EntityNode, LoadoutError, LoadoutReadObserver, LoadoutReferenceSelection,
    LoadoutResolver,
};

#[path = "loadout_reference_test_source.rs"]
mod source;
use source::{Kind, Recorder, TestResult};

fn references(spec: &EntitySpec, kind: Kind) -> Vec<&LoadoutReference> {
    match kind {
        Kind::Manual => match &spec.loadout {
            Some(LoadoutSpec::Manual(entries)) => entries
                .iter()
                .filter_map(|entry| match &entry.target {
                    TargetSpec::Reference(reference) => Some(reference),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        },
        Kind::Default => spec
            .ports
            .iter()
            .filter_map(|port| match &port.default_item {
                TargetSpec::Reference(reference) => Some(reference),
                _ => None,
            })
            .collect(),
        Kind::Assortment => match &spec.loadout {
            Some(LoadoutSpec::Assortments(references)) => references.iter().collect(),
            _ => Vec::new(),
        },
        Kind::Landing => spec
            .vehicle
            .as_ref()
            .and_then(|vehicle| vehicle.landing_system.as_ref())
            .into_iter()
            .collect(),
    }
}

fn original(tree: &RecordTree<'_>, count: usize, present: bool) {
    let actual: Vec<_> = source::values(tree)
        .into_iter()
        .filter(|value| source::guid(value).is_some())
        .collect();
    assert_eq!(
        actual.len(),
        count,
        "the unprojected canonical source preserves every actual selected slot"
    );
    for value in &actual {
        assert_eq!(
            source::guid(value).map(|guid| guid.to_string()).as_deref(),
            Some("00000000-0000-0000-0000-000000000063"),
            "each actual original GUID is checked independently before projection"
        );
        assert_eq!(
            matches!(value, Value::Reference(_)),
            present,
            "resolved versus dangling comes from encoded source membership"
        );
    }
    if let [first, second] = actual.as_slice() {
        assert!(
            !ptr::eq(*first, *second),
            "equal GUIDs occupy distinct original slots"
        );
    }
}

fn observed(
    record: &Record<'_>,
    tree: &RecordTree<'_>,
    recorder: &Recorder,
) -> Result<EntitySpec, Box<dyn std::error::Error>> {
    let mut result = None;
    assert!(
        recorder
            .with_record_paths(record, tree, &mut |observer| {
                result = Some(EntitySpec::read(tree, &PathRead::new(observer, None)));
                ControlFlow::Continue(())
            })
            .is_continue(),
        "the genuine projection runs in its borrowed original scope"
    );
    result.ok_or_else(|| "one actual observed projection".into())
}

fn pairs(spec: &EntitySpec, kind: Kind, seen: &[LoadoutReferenceSelection]) {
    let references = references(spec, kind);
    assert_eq!(
        references.len(),
        kind.count(),
        "the accepted projection retains every original slot"
    );
    assert_eq!(
        seen.len(),
        kind.count(),
        "one selection is minted per actual getter"
    );
    for (reference, selection) in references.into_iter().zip(seen) {
        assert_eq!(
            reference.selection(),
            Some(selection),
            "the exact callback pair survives the projection"
        );
        let cloned: LoadoutReference = reference.clone();
        assert_eq!(
            cloned.selection(),
            Some(selection),
            "whole-value cloning preserves the same operation"
        );
        let copied = LoadoutReference::unbound(reference.guid());
        assert_eq!(
            reference, &copied,
            "GUID equality is independent of evidence"
        );
        assert!(
            copied.selection().is_none(),
            "copying the same GUID cannot manufacture original authority"
        );
    }
    if let [first, second] = seen {
        assert_ne!(
            first, second,
            "equal GUIDs retain distinct operation identities"
        );
    }
}

#[test]
fn all_actual_guid_getters_keep_opaque_occurrences_without_changing_value_semantics() -> TestResult
{
    for kind in [Kind::Manual, Kind::Default, Kind::Assortment, Kind::Landing] {
        for present in [false, true] {
            let bytes = source::database(kind, present);
            let forge = DataForge::open(&bytes)?;
            let record = forge.record(0).ok_or("the original source Record")?;
            let tree = record.tree()?;
            original(&tree, kind.count(), present);
            let recorder = Recorder::default();
            let spec = observed(&record, &tree, &recorder)?;
            assert_eq!(
                spec,
                EntitySpec::read(&tree, &PathRead::new(None, None)),
                "evidence does not alter GUID, port, vehicle or Loadout equality"
            );
            pairs(
                &spec,
                kind,
                &recorder
                    .seen
                    .lock()
                    .map_err(|_| "the original selection log is poisoned")?,
            );
        }
    }
    Ok(())
}

fn error_selections(node: &EntityNode, kind: Kind) -> Vec<Option<&LoadoutReferenceSelection>> {
    match kind {
        Kind::Manual | Kind::Default => node
            .unresolved()
            .map(|(_, unresolved)| unresolved.reference_selection())
            .collect(),
        _ => node
            .issues()
            .iter()
            .filter_map(|error| match error {
                LoadoutError::DanglingReference { guid }
                | LoadoutError::Assortment { guid, .. } => Some(guid.selection()),
                _ => None,
            })
            .collect(),
    }
}

fn resolved_case(kind: Kind, present: bool) -> TestResult {
    let bytes = source::database(kind, present);
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("the actual root Record")?;
    let tree = record.tree()?;
    original(&tree, kind.count(), present);
    let recorder = Arc::new(Recorder::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), recorder.clone());
    let node = resolver.resolve_tree(&record, &tree)?;
    let again = resolver.resolve(&record)?;
    assert_eq!(
        node, again,
        "cache reuse preserves the original semantic result"
    );
    let seen = recorder
        .seen
        .lock()
        .map_err(|_| "the original selection log is poisoned")?;
    assert_eq!(
        seen.len(),
        kind.count(),
        "reusing a physical root projection does not mint replacement operations"
    );
    let actual = error_selections(&node, kind);
    assert_eq!(
        actual.len(),
        kind.count(),
        "every actual emitted GUID error is retained"
    );
    for (actual, original) in actual.iter().zip(seen.iter()) {
        assert_eq!(
            *actual,
            Some(original),
            "cache errors bind to the current referring occurrence instead of the first equal GUID"
        );
    }
    drop(seen);
    Ok(())
}

#[test]
fn unresolved_targets_and_cached_assortment_errors_keep_the_exact_current_selection() -> TestResult
{
    for (kind, present) in [
        (Kind::Manual, false),
        (Kind::Default, false),
        (Kind::Assortment, false),
        (Kind::Assortment, true),
        (Kind::Landing, false),
    ] {
        resolved_case(kind, present)?;
    }
    Ok(())
}

#[test]
fn reference_observer_break_refuses_the_partial_spec_and_stops_later_getters() -> TestResult {
    let bytes = source::database(Kind::Assortment, false);
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("the actual root Record")?;
    let tree = record.tree()?;
    original(&tree, 2, false);
    let recorder = Arc::new(Recorder {
        stop: true,
        ..Recorder::default()
    });
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), recorder.clone());
    assert!(
        matches!(
            resolver.resolve_tree(&record, &tree),
            Err(LoadoutError::ObservationStopped)
        ),
        "an observer refusal cannot yield an accepted prefix as a full result"
    );
    assert!(
        matches!(
            resolver.resolve(&record),
            Err(LoadoutError::ObservationStopped)
        ),
        "a stopped resolver remains stopped before reuse"
    );
    assert_eq!(
        recorder
            .seen
            .lock()
            .map_err(|_| "the original selection log is poisoned")?
            .len(),
        1,
        "the next actual slot is never delivered after refusal"
    );
    Ok(())
}

fn membership_controls(first: &RecordTree<'_>, second: &RecordTree<'_>) -> TestResult {
    let parent = source::values(first)
        .into_iter()
        .find_map(|value| match value {
            Value::Struct(parent) if parent.type_name() == "SItemPortDefaultItemDef" => {
                Some(parent)
            }
            _ => None,
        })
        .ok_or("the actual defaultItem Struct")?;
    let selected = parent
        .field("entityClass")
        .ok_or("the actual entityClass datum")?;
    assert!(
        source::belongs(first, parent, selected),
        "the original direct field has its original owner"
    );
    let copied: Value<'_> = selected.clone();
    assert!(
        !source::belongs(first, parent, &copied),
        "a copied equal Value is not an original selected slot"
    );
    assert!(
        !source::belongs(second, parent, selected),
        "another tree borrow cannot adopt the first tree's parent or value"
    );
    let parent_copy: StructValue<'_> = parent.clone();
    let copied_child = parent_copy
        .field("entityClass")
        .ok_or("the copied field is value-equal")?;
    assert!(
        !source::belongs(first, &parent_copy, copied_child),
        "a cloned parent and its equal field cannot become original members"
    );
    Ok(())
}

#[test]
fn equal_original_reads_have_distinct_selections_and_copied_values_fail_membership_controls()
-> TestResult {
    let bytes = source::database(Kind::Default, false);
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("the actual root Record")?;
    let first = record.tree()?;
    let second = record.tree()?;
    original(&first, 2, false);
    original(&second, 2, false);
    assert!(
        !ptr::eq(first.root(), second.root()),
        "the same physical Record was decoded into two actual tree borrows"
    );
    let recorder = Recorder::default();
    let first_spec = observed(&record, &first, &recorder)?;
    let second_spec = observed(&record, &second, &recorder)?;
    assert_eq!(
        first_spec, second_spec,
        "the two actual original reads have equal semantic values"
    );
    let first_refs = references(&first_spec, Kind::Default);
    let first_ref: LoadoutReference =
        (*first_refs.first().ok_or("the first original reference")?).clone();
    let second_refs = references(&second_spec, Kind::Default);
    let second_ref = second_refs.first().ok_or("the second original reference")?;
    assert_ne!(
        first_ref.selection(),
        second_ref.selection(),
        "same GUID and physical Record cannot identify another read operation"
    );
    membership_controls(&first, &second)?;
    drop(first);
    drop(second);
    assert!(
        first_ref.selection().is_some(),
        "the neutral whole pair outlives both source borrows"
    );
    Ok(())
}
