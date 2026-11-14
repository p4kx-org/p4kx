// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue};

use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};
use crate::health_snapshot_support as support;

#[test]
fn visible_sourced_value_equality_includes_completeness_and_actual_reference_membership()
-> TestResult {
    let input = Fixture::open(
        &fixture::broken_lists(1, false),
        Input::Encrypted,
        Vec::new(),
    )?;
    let (registry, scope, guid) = support::registry(&input)?;
    let payload = scope.at(support::guid_location(), SourceUseRole::Payload)?;
    let selected = scope.at(support::guid_location(), SourceUseRole::Selector)?;
    let complete = completeness(&registry, &guid, &payload);
    let selection = registry.value(SourceValue::Present(guid), &[selected], true);
    membership(&complete, &selection);
    Ok(())
}

#[test]
fn visible_sourced_value_equality_includes_actual_markers_and_independent_state() -> TestResult {
    let bytes = fixture::broken_lists(1, false);
    let plain = Fixture::open(&bytes, Input::Plain, Vec::new())?;
    let encrypted = Fixture::open(&bytes, Input::Encrypted, Vec::new())?;
    let (first, first_scope, first_guid) = support::registry(&plain)?;
    let (second, second_scope, second_guid) = support::registry(&encrypted)?;
    assert_eq!(
        first_guid, second_guid,
        "the actual original values are equal across the two byte domains"
    );
    let plain_use = first_scope.at(support::guid_location(), SourceUseRole::Payload)?;
    let encrypted_use = second_scope.at(support::guid_location(), SourceUseRole::Payload)?;
    let left = first.value(SourceValue::Present(first_guid), &[plain_use], true);
    let right = second.value(SourceValue::Present(second_guid), &[encrypted_use], true);
    markers(&left, &right);
    independent_states(&first);
    Ok(())
}

fn independent_states(registry: &SourceUses) {
    let absent = registry.value::<String>(SourceValue::Absent, &[], true);
    let null = registry.value::<String>(SourceValue::Null, &[], true);
    let empty = registry.value(SourceValue::Present(String::new()), &[], true);
    let literal = registry.value(SourceValue::Present("literal".to_owned()), &[], true);
    assert_eq!(
        absent.value(),
        null.value(),
        "both independent non-present control values have no payload"
    );
    assert_eq!(
        absent.origin(),
        null.origin(),
        "the independent control origins are equal"
    );
    assert_ne!(
        absent, null,
        "the visible source state is part of typed equality"
    );
    assert_eq!(
        empty.origin(),
        literal.origin(),
        "independent literal values share the same control origin"
    );
    assert_ne!(
        empty, literal,
        "typed equality preserves an exact present empty string"
    );
}

#[test]
fn equal_visible_inventory_values_never_restore_a_foreign_original_use_authority() -> TestResult {
    let bytes = fixture::broken_lists(1, false);
    let left = Fixture::open(&bytes, Input::Plain, Vec::new())?;
    let right = Fixture::open(&bytes, Input::Plain, Vec::new())?;
    let (first, first_scope, first_guid) = support::registry(&left)?;
    let (second, second_scope, second_guid) = support::registry(&right)?;
    assert_eq!(
        first_guid, second_guid,
        "actual headers are equal without sharing original authority"
    );
    let original = first_scope.at(support::guid_location(), SourceUseRole::Payload)?;
    let actual = second_scope.at(support::guid_location(), SourceUseRole::Payload)?;
    let first_snapshot = first.freeze()?;
    let second_snapshot = second.freeze()?;
    assert_eq!(
        first_snapshot, second_snapshot,
        "typed descriptor equality excludes only private physical bindings"
    );
    assert!(
        !first.is_same_inventory(&second),
        "equal values cannot merge two original registries"
    );
    foreign_authority(&second, &second_guid, &original, &actual);
    support::closed(&second_snapshot);
    Ok(())
}

fn completeness(registry: &SourceUses, guid: &str, payload: &SourceUse) -> SourcedValue<String> {
    let complete = registry.value(
        SourceValue::Present(guid.to_owned()),
        std::slice::from_ref(payload),
        true,
    );
    let incomplete = registry.value(
        SourceValue::Present(guid.to_owned()),
        std::slice::from_ref(payload),
        false,
    );
    assert_eq!(
        complete.value(),
        incomplete.value(),
        "only contribution completeness changes the visible datum"
    );
    assert_eq!(
        complete.origin().contains_encrypted_source(),
        Some(true),
        "the actual encrypted witness is known"
    );
    assert_eq!(
        incomplete.origin().contains_encrypted_source(),
        Some(true),
        "known encryption survives incomplete capture"
    );
    assert_eq!(
        complete.origin().source_uses_refs(),
        incomplete.origin().source_uses_refs(),
        "the same original use remains retained"
    );
    assert_ne!(
        complete, incomplete,
        "typed value equality includes visible contribution completeness"
    );
    complete
}

fn membership(complete: &SourcedValue<String>, selection: &SourcedValue<String>) {
    assert_eq!(
        complete.value(),
        selection.value(),
        "both actual operations retain the same original GUID"
    );
    assert_eq!(
        complete.origin().contains_encrypted_source(),
        selection.origin().contains_encrypted_source(),
        "reference membership is independent of the encryption flag"
    );
    assert_eq!(
        complete.origin().source_set_complete(),
        selection.origin().source_set_complete(),
        "both actual contribution sets are complete"
    );
    assert_ne!(
        complete.origin().source_uses_refs(),
        selection.origin().source_uses_refs(),
        "the actual original operations have distinct use IDs"
    );
    assert_ne!(
        complete, selection,
        "typed equality includes the visible source references"
    );
}

fn markers(left: &SourcedValue<String>, right: &SourcedValue<String>) {
    assert_eq!(
        left.value(),
        right.value(),
        "the original retained value does not encode lineage"
    );
    assert_eq!(
        left.origin().source_uses_refs(),
        right.origin().source_uses_refs(),
        "document-local IDs can coincide across actual inputs"
    );
    assert_eq!(
        left.origin().source_set_complete(),
        right.origin().source_set_complete(),
        "both original sets were captured completely"
    );
    assert_eq!(
        left.origin().contains_encrypted_source(),
        Some(false),
        "the real plain payload remains plain"
    );
    assert_eq!(
        right.origin().contains_encrypted_source(),
        Some(true),
        "the real encrypted payload remains encrypted"
    );
    assert_ne!(
        left, right,
        "typed equality includes the actual encryption marker"
    );
}

fn foreign_authority(registry: &SourceUses, guid: &str, original: &SourceUse, actual: &SourceUse) {
    let foreign = registry.value(
        SourceValue::Present(guid.to_owned()),
        std::slice::from_ref(original),
        true,
    );
    let own = registry.value(
        SourceValue::Present(guid.to_owned()),
        std::slice::from_ref(actual),
        true,
    );
    assert_eq!(
        foreign.origin().contains_encrypted_source(),
        None,
        "a foreign handle cannot certify the actual current input"
    );
    assert!(
        !foreign.origin().source_set_complete(),
        "foreign authority remains explicitly incomplete"
    );
    assert!(
        foreign.origin().source_uses_refs().is_empty(),
        "matching descriptor IDs cannot attach a foreign original contribution"
    );
    assert_eq!(
        own.origin().contains_encrypted_source(),
        Some(false),
        "the genuine own plain observation remains proved"
    );
    assert!(
        own.origin().source_set_complete(),
        "the actual original contribution is complete"
    );
}
