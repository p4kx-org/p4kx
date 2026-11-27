// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceEvidenceError, SourceValue};

use crate::CatalogError;
use crate::build::original_input::test_support::{self, TestResult};

use super::fixture;

#[test]
fn a_selected_arena_cannot_bind_to_a_second_report_over_the_same_physical_input() -> TestResult {
    test_support::with_same_forge_reports(&fixture::bytes(), |forge, input, foreign| {
        let inputs = fixture::selected(forge, input)?;
        let before = foreign.origins().uses.freeze()?;
        let result = inputs.bind(
            SourceValue::Present(fixture::CALLERS),
            &foreign.origins().uses,
        );
        assert!(
            matches!(
                result,
                Err(CatalogError::SourceEvidence(
                    SourceEvidenceError::ForeignEvidence
                ))
            ),
            "equal physical sources keep separate report inventories"
        );
        assert_eq!(
            before,
            foreign.origins().uses.freeze()?,
            "the refused foreign binding emits no observations"
        );
        Ok(())
    })
}

#[test]
fn two_genuine_builds_cannot_merge_their_distinct_selected_arenas() -> TestResult {
    test_support::with_same_forge_reports(&fixture::bytes(), |forge, input, _| {
        let mut first = fixture::selected(forge, input)?;
        let second = fixture::selected(forge, input)?;
        let sources = &input.origins().uses;
        let before = serde_json::to_value(
            first
                .clone()
                .bind(SourceValue::Present(fixture::CALLERS), sources)?,
        )?;
        assert!(
            matches!(
                first.merge(second),
                Err(CatalogError::SourceEvidence(
                    SourceEvidenceError::ForeignEvidence
                ))
            ),
            "two equal selected trees are separate original build arenas"
        );
        assert_eq!(
            serde_json::to_value(first.bind(SourceValue::Present(fixture::CALLERS), sources,)?)?,
            before,
            "a failed arena merge preserves the original retained cell"
        );
        Ok(())
    })
}
