// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use super::{Shape, at, database, enter};
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::ReferenceCapture;
use crate::catalog_read::original::health_example_body::{ExampleBody, ExampleBodyReceipt};
use crate::catalog_read::original::health_example_retention::RetainedExampleBody;
use crate::record_observation::RecordObservation;

#[test]
fn compact_field_samples_keep_exact_cells_and_ordered_merge_operands() -> TestResult {
    assert!(
        size_of::<ExampleBodyReceipt>() < size_of::<ExampleBody>(),
        "field paths occupy less inline storage than the full receipt"
    );

    test_support::with_input(&database(Shape::Struct), |forge, input| {
        let record = forge.record(0).ok_or("the actual Owner is absent")?;
        let tree = record.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let admission = RecordObservation::capture(input, &record)?;
        let mut capture = ReferenceCapture::new(input, &admission, &scope)?;
        let parent = enter(&scope, &mut capture, "left")?;
        let sources = &input.origins().uses;
        let before = sources.freeze()?;
        let (eager, compact) = at(&scope, "left.empty", |site| {
            let body = capture.body(site)?;
            let eager = ExampleBodyReceipt::captured(
                Arc::clone(&capture.frame),
                body.field,
                body.source_type,
                body.target_guid,
            );
            Ok((eager, capture.example(site)?))
        })?;
        capture.leave(parent)?;
        let eager_merged = merged(eager.clone())?;
        let compact_merged = merged(compact.clone())?;
        assert_eq!(
            before,
            sources.freeze()?,
            "capture, cloning and equal-key merge register no report uses"
        );

        let single = RetainedExampleBody::read(eager, input)?;
        let combined = RetainedExampleBody::read(eager_merged, input)?;
        let expected = sources.freeze()?;
        let deferred = RetainedExampleBody::read(compact, input)?;
        let settled = RetainedExampleBody::read(compact_merged, input)?;
        assert_eq!(
            expected,
            sources.freeze()?,
            "compact examples register exactly the eager physical uses"
        );

        compare_contributions(&single, &deferred);
        compare_contributions(&combined, &settled);
        let examples = [single, deferred, combined, settled];
        let facts: Vec<_> = examples
            .iter()
            .flat_map(RetainedExampleBody::texts)
            .collect();
        let texts = input.origins().health_texts().seal(sources, None, &facts)?;
        let [single, deferred, combined, settled] = examples;
        for (eager, compact) in [(single, deferred), (combined, settled)] {
            assert_eq!(
                serde_json::to_vec(&eager.finish(&texts, sources)?)?,
                serde_json::to_vec(&compact.finish(&texts, sources)?)?,
                "report cells retain exact text positions, source flags, and proof references"
            );
        }
        Ok(())
    })
}

fn merged(mut body: ExampleBodyReceipt) -> TestResult<ExampleBodyReceipt> {
    body.merge(body.clone())?;
    Ok(body)
}

fn compare_contributions(expected: &RetainedExampleBody, actual: &RetainedExampleBody) {
    for (expected, actual) in expected.texts().into_iter().zip(actual.texts()) {
        let (expected, expected_complete) = expected.contributions();
        let (actual, actual_complete) = actual.contributions();
        assert_eq!(
            expected_complete, actual_complete,
            "the stored recipe retains the same proof coverage"
        );
        assert!(
            expected
                .iter()
                .map(|used| (used.reference(), used.role()))
                .eq(actual.iter().map(|used| (used.reference(), used.role()))),
            "the complete original coordinate and role sequence is unchanged"
        );
    }
}
