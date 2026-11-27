// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::item_recovery::{
    ItemRecoveryOverrides, ItemRecoverySnapshot, RecoveryConditionPayload, RecoveryEconomy,
    with_item_recovery,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability, ResourceNumber,
};
use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::{Language, SourceValueState};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

use super::super::{ITEM_RECOVERY_ECONOMY, ITEM_RECOVERY_OVERRIDES, ITEM_RECOVERY_POLICY};
use super::{Route, TestResult};

#[path = "sdk_item_recovery_fixture.rs"]
mod fixture;

use fixture::{TEXT, archive, damaged_database, full_database, outside_database};

const ALPHA_PATH: &str = "libs/foundry/records/entitlementpolicies/alpha";
const ALPHA_INDEX: u32 = 1;
const FIRST_READABLE_INDEX: u32 = 3;

pub(super) fn read() -> TestResult<[Route; 3]> {
    let (_folder, archive) = archive(&full_database(), true)?;
    let full = build_catalog(
        &archive,
        CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
    )?;
    let full: Value = serde_json::from_slice(full.json())?;
    full_proof::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
    )?
    .with_item_recovery(CatalogRecordSelection::All, None, |snapshot| {
        verify_policy(&snapshot)?;
        Ok([
            parity(
                &full,
                snapshot.availability,
                snapshot.economy.as_slice(),
                ITEM_RECOVERY_ECONOMY,
            )?,
            parity(
                &full,
                snapshot.availability,
                snapshot.overrides.as_slice(),
                ITEM_RECOVERY_OVERRIDES,
            )?,
            parity(
                &full,
                snapshot.availability,
                snapshot.policy.as_slice(),
                ITEM_RECOVERY_POLICY,
            )?,
        ])
    })?
}

fn parity<Row: Serialize + JsonSchema>(
    full: &Value,
    availability: DatasetAvailability,
    rows: &[Row],
    dataset: &'static str,
) -> TestResult<Route> {
    assert_eq!(
        full.pointer(dataset),
        Some(&json!({"availability": availability, "rows": rows})),
        "selected original policy projection matches Catalog at {dataset}"
    );
    Route::queried_rows(dataset, rows, None)
}

fn verify_policy(snapshot: &ItemRecoverySnapshot<'_>) -> TestResult {
    let economy = snapshot.economy.as_ref().ok_or("the original economy")?;
    let overrides = snapshot
        .overrides
        .as_ref()
        .ok_or("the original overrides")?;
    let policy = snapshot
        .policy
        .as_ref()
        .ok_or("the original set conditions")?;
    assert_eq!(
        economy.source.record_path().value().copied(),
        Some(ALPHA_PATH),
        "the earliest original Record path wins over the earlier directory position"
    );
    assert_eq!(
        overrides.owner_record_index.value(),
        Some(&ALPHA_INDEX),
        "the override table uses the same owner"
    );
    assert_eq!(
        policy.owner_record_index.value(),
        Some(&ALPHA_INDEX),
        "the policy table uses the same owner"
    );
    verify_economy(
        economy
            .economy_params
            .value()
            .ok_or("the original economy block")?,
    );
    verify_overrides(overrides)?;
    verify_notification(snapshot)
}

fn verify_overrides(overrides: &ItemRecoveryOverrides) -> TestResult {
    let [group] = overrides
        .overrides
        .value()
        .ok_or("the original override slots")?
        .as_slice()
    else {
        return Err("one original override".into());
    };
    let classes = group
        .value()
        .ok_or("the authored override")?
        .classes
        .value()
        .ok_or("the original class array")?;
    let [class, absent] = classes.as_slice() else {
        return Err("one class reference and a null slot".into());
    };
    assert_eq!(
        class
            .value()
            .and_then(|class| class.target_type().value())
            .map(String::as_str),
        Some("EntityClassDefinition"),
        "the original class selector retains its target header"
    );
    assert_eq!(
        absent.state(),
        SourceValueState::Null,
        "null class positions survive"
    );
    Ok(())
}

fn verify_economy(economy: &RecoveryEconomy) {
    let multiplier = &economy.legacy.claim_cost_multiplier;
    assert!(
        matches!(multiplier.value(), Some(ResourceNumber::Single(value)) if value.raw_bits() == 0.25_f32.to_bits()),
        "the active owner's original floating-point bits survive"
    );
    assert_eq!(
        multiplier.origin().contains_encrypted_source(),
        Some(true),
        "the value comes from encrypted DataForge"
    );
}

fn verify_notification(snapshot: &ItemRecoverySnapshot<'_>) -> TestResult {
    let policy = snapshot.policy.as_ref().ok_or("the original policy")?;
    let [class, absent] = policy
        .insurable
        .value()
        .ok_or("original condition slots")?
        .as_slice()
    else {
        return Err("one class condition and a null slot".into());
    };
    assert!(
        matches!(
            class.value().map(|condition| &condition.payload),
            Some(RecoveryConditionPayload::EntityClass { class_def }) if class_def.value().is_some()
        ),
        "the authored class condition retains a typed original Record link"
    );
    assert_eq!(
        absent.state(),
        SourceValueState::Null,
        "the original null condition remains at its slot"
    );
    let notices = policy
        .notification_params
        .value()
        .ok_or("the original notification settings")?;
    let reference = notices
        .item_bricked_title
        .value()
        .ok_or("the original notification key")?;
    let text = snapshot
        .game_text(reference)
        .ok_or("registered protected notification")?;
    let translations = text
        .translations()
        .value()
        .ok_or("consulted translation domain")?;
    let english = translations
        .get("en")
        .ok_or("requested English translation")?;
    assert_eq!(
        english.source_text.value().map(std::borrow::Cow::as_ref),
        Some(TEXT),
        "the notification resolves in this query result"
    );
    assert_eq!(
        snapshot.available_languages,
        [Language::English],
        "the query only attaches the requested language"
    );
    Ok(())
}

#[test]
fn exact_recovery_scope_skips_an_unreadable_earlier_policy_and_an_outsider() -> TestResult {
    let (_folder, archive) = archive(&damaged_database(), false)?;
    with_item_recovery(
        &archive,
        CatalogRecordSelection::RecordIndex(FIRST_READABLE_INDEX),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            assert!(
                snapshot.coverage.unreadable_bodies.is_empty(),
                "exact selection does not read other policy bodies"
            );
            assert!(
                snapshot.coverage.unreadable_headers.is_empty(),
                "the selected owner header is readable"
            );
            assert_eq!(
                snapshot
                    .policy
                    .and_then(|policy| policy.owner_record_index.value().copied()),
                Some(FIRST_READABLE_INDEX),
                "record-index selection reads that physical owner despite a duplicate GUID"
            );
        },
    )?;
    with_item_recovery(
        &archive,
        CatalogRecordSelection::Guid("00000000-0000-0000-0000-000000000010".parse()?),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.coverage.unreadable_bodies,
                [0],
                "the damaged matching owner remains in coverage"
            );
            assert_eq!(
                snapshot
                    .policy
                    .and_then(|policy| policy.owner_record_index.value().copied()),
                Some(FIRST_READABLE_INDEX),
                "equal paths select the first readable physical owner"
            );
        },
    )?;
    Ok(())
}

#[test]
fn exact_recovery_scope_does_not_read_an_outside_policy_record() -> TestResult {
    let (_folder, archive) = archive(&outside_database(), true)?;
    with_item_recovery(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
        |snapshot| {
            assert!(
                snapshot.policy.is_none(),
                "the exact scope still requires the entitlement-policy folder"
            );
            assert!(
                snapshot.coverage.unreadable_bodies.is_empty(),
                "the outside body is never attempted"
            );
            assert!(
                snapshot.available_languages.is_empty(),
                "without a selected owner no localization is loaded"
            );
            assert!(
                matches!(snapshot.availability, DatasetAvailability::Available),
                "the archive still declares the family"
            );
        },
    )?;
    Ok(())
}
