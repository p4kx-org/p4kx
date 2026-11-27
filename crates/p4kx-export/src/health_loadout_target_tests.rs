// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourceUseRole, Value, ValueError};

use crate::build::original_input::ship_pool_test_input;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::health_example_retention::RetainedExampleBody;
use crate::catalog_read::original::{
    EntryPathCapture, ExampleBodyReceipt, OriginalFlowFields, OriginalValuePart,
};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

use super::LoadoutTargetCapture;

#[path = "health_loadout_target_fixture.rs"]
mod fixture;

#[test]
fn genuine_equal_dangling_slots_keep_both_original_guid_payloads_after_typed_merge() -> TestResult {
    ship_pool_test_input::with_input(&fixture::database(), Vec::new(), |_, _, forge, input| {
        let original = fixture::body(forge, input, &[(0, 2)])?;
        let locations = fixture::retained_locations(original, input)?;
        assert_eq!(
            locations.len(),
            2,
            "both independently observed original GUID words are retained"
        );
        assert_ne!(
            locations.first(),
            locations.get(1),
            "equal GUID values keep distinct original occurrence coordinates"
        );
        assert!(
            locations.iter().all(|location| matches!(location,
            SourceLocation::Subresource { indices, .. } if indices.first() == Some(&0))),
            "both retained GUID words belong to the genuine original root referrer"
        );
        Ok(())
    })
}

#[test]
fn equal_targets_from_different_referrer_records_preserve_both_original_frames() -> TestResult {
    ship_pool_test_input::with_input(
        &fixture::multiple_referrers(),
        Vec::new(),
        |_, _, forge, input| {
            let original = fixture::body(forge, input, &[(0, 0), (1, 1), (2, 1)])?;
            let locations = fixture::retained_locations(original, input)?;
            assert_eq!(
                locations.len(),
                2,
                "the root sample keeps both genuine child-Record GUID contributions"
            );
            for expected in [1, 2] {
                assert!(
                    locations.iter().any(|location| matches!(location,
                SourceLocation::Subresource { indices, .. } if indices.first() == Some(&expected))),
                    "the same root diagnostic retains each actual child referrer's coordinate"
                );
            }
            Ok(())
        },
    )
}

#[test]
fn target_capture_refuses_a_second_scope_of_the_same_original_tree_before_payload() -> TestResult {
    test_support::with_input(&fixture::database(), |forge, input| {
        let record = forge.record(0).ok_or("the actual original owner")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(input, &record)?;
        let first = input.record_scope(&record, &tree)?;
        let second = input.record_scope(&record, &tree)?;
        let paths = EntryPathCapture::new(input, &admission, &first)?;
        let capture = LoadoutTargetCapture::from_paths(&paths);
        let fields = OriginalFlowFields::read(&second, None)?;
        let fact = fields
            .facts()
            .iter()
            .find(|fact| {
                matches!(
                    fact.value(),
                    Value::Error(ValueError::DanglingReference { .. })
                )
            })
            .ok_or("the genuine original second-scope Reference fact")?;
        let ancestors = fields.ancestors(fact)?;
        let parent = fields.guard_parent(fact)?;
        let before = input.origins().uses.freeze()?;
        let outcome = capture.value_fact(parent, fact, &ancestors);
        assert!(
            matches!(
                outcome,
                Err(CatalogError::Accounting(
                    ExportAccountingError::ConflictingSourceDatum
                ))
            ),
            "same Record/tree values do not rebind the exact borrowed capture scope"
        );
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "foreign-scope rejection registers no payload"
        );
        Ok(())
    })
}

#[test]
fn captured_target_refuses_an_independent_report_without_retaining_payload() -> TestResult {
    ship_pool_test_input::with_input(&fixture::database(), Vec::new(), |_, _, forge, input| {
        let original = fixture::body(forge, input, &[(0, 2)])?;
        ship_pool_test_input::with_input(
            &fixture::database(),
            Vec::new(),
            |_, _, forge, foreign| {
                let own = fixture::body(forge, foreign, &[(0, 2)])?;
                assert_eq!(
                    original.description()?,
                    own.description()?,
                    "equal values do not pair independent reports"
                );
                let before = foreign.origins().uses.freeze()?;
                let outcome = RetainedExampleBody::read(original, foreign);
                assert!(
                    matches!(
                        outcome,
                        Err(CatalogError::Accounting(
                            ExportAccountingError::UnboundHealthSubject
                                | ExportAccountingError::ConflictingSourceDatum
                        ))
                    ),
                    "the captured actual foreign owner cannot authorize this report's retained body"
                );
                assert_eq!(
                    before,
                    foreign.origins().uses.freeze()?,
                    "foreign refusal registers no payload"
                );
                Ok(())
            },
        )
    })
}

fn reject_copy<'source>(fields: &OriginalFlowFields<'_, '_, 'source>, value: &Value<'source>) {
    let copied = value.clone();
    assert!(
        matches!(
            fields.fact(&copied),
            Err(CatalogError::Accounting(
                ExportAccountingError::ConflictingSourceDatum
            ))
        ),
        "a copied equal Value cannot acquire the original stored Fact"
    );
}

#[test]
fn resolved_reference_target_keeps_the_authored_guid_word_and_rejects_copied_value() -> TestResult {
    test_support::with_input(&fixture::multiple_referrers(), |forge, input| {
        let record = forge.record(0).ok_or("the original root referrer")?;
        let tree = record.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let admission = RecordObservation::capture(input, &record)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let fact = fields
            .facts()
            .iter()
            .find(|fact| matches!(fact.value(), Value::Reference(_)))
            .ok_or("the actual resolved Reference value")?;
        let Value::Reference(original) = fact.value() else {
            return Err("the original field must remain a resolved Reference".into());
        };
        let expected = original.guid();
        let expected_location = fact.location(OriginalValuePart::Value);
        reject_copy(&fields, fact.value());
        let paths = EntryPathCapture::new(input, &admission, &scope)?;
        let capture = LoadoutTargetCapture::from_paths(&paths);
        let before = input.origins().uses.freeze()?;
        let target =
            capture.value_fact(fields.guard_parent(fact)?, fact, &fields.ancestors(fact)?)?;
        let body = ExampleBodyReceipt::loadout_target(input, admission.subject().clone(), target)?;
        assert_eq!(
            body.description()?.2,
            Some(expected),
            "the genuine original target GUID is unchanged"
        );
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "unretained resolved target registers no payload"
        );
        let _retained = RetainedExampleBody::read(body, input)?;
        let inventory = input.origins().uses.freeze()?;
        let payloads: Vec<_> = inventory
            .uses()
            .iter()
            .filter(|used| used.role() == SourceUseRole::Payload && !before.uses().contains(used))
            .collect();
        assert_eq!(
            payloads.len(),
            1,
            "the retained Reference adds exactly its authored GUID word beyond the genuine observed prefix"
        );
        assert_eq!(
            payloads.first().ok_or("the retained GUID word")?.location(),
            &expected_location,
            "the retained target GUID uses the exact original authored occurrence"
        );
        Ok(())
    })
}
