// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};
use p4kx_export::{CatalogError, build_catalog_from};
use p4kx_extraction::{
    InputScope, SourceInput, SourceLocation, SourceUse, SourceUseRole, SourceUses, SourceValue,
};
use serde_json::{Value, json};

#[test]
fn equal_visible_inventories_never_authorize_foreign_original_health_inputs() -> TestResult {
    let bytes = fixture::broken_lists(1, false);
    let first = Fixture::open(&bytes, Input::Plain, Vec::new())?;
    let second = Fixture::open(&bytes, Input::Plain, Vec::new())?;
    original_membership(&first, &second)?;
    separate_registries(&first, &second)?;
    assert!(
        matches!(
            build_catalog_from(&second.archive, &first.entry, &fixture::options()),
            Err(CatalogError::SourceOriginMismatch)
        ),
        "equal bytes and visible inventories cannot retarget an original DCB into another Archive"
    );
    Ok(())
}

fn original_membership(first: &Fixture, second: &Fixture) -> TestResult {
    let first_entry = first
        .archive
        .entries()
        .first()
        .ok_or("first original DCB Entry")?;
    let second_entry = second
        .archive
        .entries()
        .first()
        .ok_or("second original DCB Entry")?;
    assert_eq!(
        first_entry.index(),
        second_entry.index(),
        "physical ordinals can coincide in different original inputs"
    );
    assert_eq!(
        first_entry.path(),
        second_entry.path(),
        "visible paths can coincide without shared ownership"
    );
    assert!(
        second.archive.observed_entry(first_entry).is_none(),
        "equal descriptors cannot become an original observation in another input"
    );
    let detached = (*first_entry).clone();
    assert!(
        first.archive.observed_entry(&detached).is_none(),
        "a detached equal-valued clone is not the original admitted slot"
    );
    for input in [first, second] {
        assert_eq!(
            input
                .entry
                .open()?
                .record(0)
                .ok_or("original header owner")?
                .guid()
                .to_string(),
            fixture::guid(fixture::LIST),
            "both actual original headers carry the same GUID"
        );
    }
    Ok(())
}

fn registry(input: &Fixture) -> TestResult<(SourceUses, SourceUse)> {
    let entry = input
        .archive
        .entries()
        .first()
        .ok_or("actual original DCB Entry")?;
    let sources = SourceUses::new();
    let scope = sources.observe(
        SourceInput::ArchiveView(
            input
                .archive
                .observed_entry(entry)
                .ok_or("actual original observation")?,
        ),
        InputScope::Payload,
    )?;
    let location = SourceLocation::Subresource {
        namespace: "dataforgeRecordHeader".to_owned(),
        indices: vec![0],
        field: "guid".to_owned(),
    };
    let contribution = scope.at(location, SourceUseRole::Payload)?;
    Ok((sources, contribution))
}

fn separate_registries(first: &Fixture, second: &Fixture) -> TestResult {
    let (first_registry, first_contribution) = registry(first)?;
    let (second_registry, second_contribution) = registry(second)?;
    let visible_first = serde_json::to_value(first_registry.freeze()?)?;
    let visible_second = serde_json::to_value(second_registry.freeze()?)?;
    assert_eq!(
        visible_first, visible_second,
        "equal visible frozen inventories still hide distinct original authority"
    );
    assert!(
        !first_registry.is_same_inventory(&second_registry),
        "structural equality does not merge the original registries"
    );
    contributions(&second_registry, first_contribution, second_contribution)
}

fn contributions(sources: &SourceUses, foreign_use: SourceUse, own_use: SourceUse) -> TestResult {
    let foreign = sources.value(
        SourceValue::Present(fixture::guid(fixture::LIST)),
        &[foreign_use],
        true,
    );
    let own = sources.value(
        SourceValue::Present(fixture::guid(fixture::LIST)),
        &[own_use],
        true,
    );
    let foreign: Value = serde_json::to_value(foreign)?;
    let own: Value = serde_json::to_value(own)?;
    assert_eq!(
        foreign.get("containsEncryptedSource"),
        Some(&Value::Null),
        "foreign contributions cannot certify an original input"
    );
    assert_eq!(
        foreign.get("sourceSetComplete"),
        Some(&json!(false)),
        "foreign authority stays explicitly incomplete"
    );
    assert_eq!(
        foreign.get("sourceUsesRefs"),
        Some(&json!([])),
        "no foreign use is attached by matching descriptor text"
    );
    assert_eq!(
        own.get("containsEncryptedSource"),
        Some(&json!(false)),
        "the actual own original plain use remains proved"
    );
    Ok(())
}
