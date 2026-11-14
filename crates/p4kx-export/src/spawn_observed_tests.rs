// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, SourceEvidenceError, TagTree, Value, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::Reader;
use crate::CatalogError;
use crate::build::original_input::test_support::{TestResult, with_input};
use crate::catalog_read::original::OriginalFlowFields;
use crate::record_observation::RecordObservation;
use crate::record_read::ReadError;
use crate::source_fields::{FieldError, FieldIssue};
use crate::template_field_capture::TemplateFieldCapture;

#[test]
fn observed_reader_keeps_the_original_unreadable_guard_body_deferred() -> TestResult {
    with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original owner")?;
        let tree = record.tree()?;
        assert_eq!(
            tree.field("flag"),
            Some(&Value::Error(ValueError::InvalidBool { byte: 3 })),
            "actual canonical source error exists before the observed guard"
        );
        let scope = input.record_scope(&record, &tree)?;
        let original = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&original, input, &admission)?;
        let tags = TagTree::default();
        let reader = Reader::for_observed(&tags, &tree, &capture).map_err(plain)?;
        let before = input.origins().uses.freeze()?;
        let error = reader
            .fields(Some(tree.root()))
            .strict_number("flag", "")
            .err()
            .ok_or("actual unreadable strict guard")?;
        let error = error
            .downcast::<FieldError>()
            .map_err(|_| "original domain error type")?;
        assert!(
            matches!(error.description().issue, FieldIssue::Unreadable),
            "original unreadable category"
        );
        let body = error.body().ok_or("actual guard body recipe")?;
        body.check_description(Some("flag"), Some("Owner"), None)?;
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "creating the error recipe claims no early payload authority"
        );
        Ok(())
    })
}

#[test]
fn observed_constructor_rejects_an_equal_second_tree_from_the_same_record() -> TestResult {
    with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original owner")?;
        let tree = record.tree()?;
        let second = record.tree()?;
        assert!(
            tree.is_from(&record) && second.is_from(&record),
            "both are genuine reads of the same Record"
        );
        assert_eq!(
            tree.fields(),
            second.fields(),
            "equal source values do not establish the same frame"
        );
        let scope = input.record_scope(&record, &tree)?;
        let original = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&original, input, &admission)?;
        let before = input.origins().uses.freeze()?;
        let tags = TagTree::default();
        let error = Reader::for_observed(&tags, &second, &capture)
            .err()
            .ok_or("a foreign original tree must be rejected")?;
        assert!(
            matches!(
                typed(error)?,
                CatalogError::SourceEvidence(SourceEvidenceError::ForeignEvidence)
            ),
            "same-Record metadata cannot authorize another actual tree"
        );
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "foreign construction claims no source facts"
        );
        Ok(())
    })
}

#[test]
fn observed_permissive_getters_do_not_swallow_a_foreign_parent_as_absence() -> TestResult {
    with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original owner")?;
        let tree = record.tree()?;
        let second = record.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let original = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&original, input, &admission)?;
        let tags = TagTree::default();
        let reader = Reader::for_observed(&tags, &tree, &capture).map_err(plain)?;
        let before = input.origins().uses.freeze()?;
        let error = reader
            .fields(Some(second.root()))
            .boolean("flag")
            .err()
            .ok_or("foreign parent must fail before a permissive getter")?;
        assert!(
            matches!(
                typed(error)?,
                CatalogError::SourceEvidence(SourceEvidenceError::ForeignEvidence)
            ),
            "typed source admission failure is not a nullable field result"
        );
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "foreign getter claims no source facts"
        );
        Ok(())
    })
}

#[test]
fn observed_getters_keep_actual_cancellation_as_a_typed_fatal_error() -> TestResult {
    with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original owner")?;
        let tree = record.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let cancel = CancelToken::new();
        let original = OriginalFlowFields::read(&scope, Some(&cancel))?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&original, input, &admission)?;
        let tags = TagTree::default();
        let reader = Reader::for_observed(&tags, &tree, &capture).map_err(plain)?;
        let before = input.origins().uses.freeze()?;
        cancel.cancel();
        let error = reader
            .fields(Some(tree.root()))
            .boolean("flag")
            .err()
            .ok_or("actual cancelled field access")?;
        assert!(
            matches!(typed(error)?, CatalogError::Cancelled),
            "cancellation cannot become a successful absent value"
        );
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "cancelled getter claims no source facts"
        );
        Ok(())
    })
}

fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Owner").field("flag", TestType::Bool))
        .record(
            TestRecord::new(0xe117, "Owner", "Owner.Source", "source.xml")
                .value("flag", TestValue::Raw(vec![3])),
        )
        .build()
}

fn plain(error: ReadError) -> Box<dyn std::error::Error> {
    error
}

fn typed(error: ReadError) -> TestResult<CatalogError> {
    error
        .downcast::<CatalogError>()
        .map(|error| *error)
        .map_err(|_| "original typed CatalogError was lost".into())
}
