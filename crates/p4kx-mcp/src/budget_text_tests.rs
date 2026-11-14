// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::{Display, Error as FormatError, Formatter, Result as FormatResult};

use p4kx_assembly::LoadoutError;

use crate::budget::{Budget, MAX_RESPONSE_LEN};
use crate::error::ToolError;

struct Chunks;

impl Display for Chunks {
    fn fmt(&self, output: &mut Formatter<'_>) -> FormatResult {
        output.write_str(" é")?;
        output.write_str("\\n%% ")?;
        output.write_str("界")
    }
}

struct Broken;

impl Display for Broken {
    fn fmt(&self, _: &mut Formatter<'_>) -> FormatResult {
        Err(FormatError)
    }
}

struct IgnoresFailure<'text>(&'text str);

impl Display for IgnoresFailure<'_> {
    fn fmt(&self, output: &mut Formatter<'_>) -> FormatResult {
        let _failure = output.write_str(self.0);
        Ok(())
    }
}

#[test]
fn bounded_display_preserves_original_chunks_and_utf8_lengths() -> Result<(), ToolError> {
    let mut budget = Budget::new();
    let expected = " é\\n%% 界";
    assert_eq!(
        super::display(&mut budget, &Chunks)?,
        expected,
        "no display text is normalized"
    );
    assert_eq!(
        budget.left(),
        MAX_RESPONSE_LEN - expected.len(),
        "exact UTF-8 output bytes"
    );
    Ok(())
}

#[test]
fn bounded_display_real_loadout_reason_is_refused_without_huge_error_metadata() {
    let value = LoadoutError::UnknownClass {
        name: "M".repeat(300 << 10),
    };
    let error = super::display(&mut Budget::new(), &value)
        .expect_err("actual formatted reason exceeds budget");
    assert!(
        matches!(error, ToolError::TooLargeItem { .. }),
        "actual write admission refused"
    );
    assert!(
        error.message().len() < 1024,
        "refusal metadata does not clone the huge name"
    );
}

#[test]
fn bounded_display_preserves_an_independent_formatter_failure() {
    let mut budget = Budget::new();
    let error = super::display(&mut budget, &Broken).expect_err("the formatter actually fails");
    assert!(
        matches!(error, ToolError::ResponseTextFormat { .. }),
        "a genuine format failure is not TooLargeItem"
    );
    assert_eq!(budget.left(), MAX_RESPONSE_LEN, "no chunk was written");
}

#[test]
fn bounded_display_cannot_return_success_after_a_latched_write_refusal() {
    let mut budget = Budget::new();
    assert!(budget.take(MAX_RESPONSE_LEN - 1), "leave one byte");
    let error =
        super::display(&mut budget, &IgnoresFailure("é")).expect_err("refusal stays latched");
    assert!(
        matches!(error, ToolError::TooLargeItem { .. }),
        "formatter success cannot hide incomplete output"
    );
    assert_eq!(
        budget.left(),
        1,
        "the refused multi-byte scalar was not partially copied"
    );
}
