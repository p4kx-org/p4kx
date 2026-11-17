// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceEncryptionDeclaration, SourcedValue};

use super::{Comparison, TestResult};

pub(super) fn declarations(
    comparison: &Comparison<'_, '_>,
    expected: &SourcedValue<Vec<SourcedValue<SourceEncryptionDeclaration>>>,
    actual: &SourcedValue<Vec<SourcedValue<SourceEncryptionDeclaration>>>,
) -> TestResult {
    comparison.origin(expected.origin(), actual.origin())?;
    if let (Some(expected), Some(actual)) = (expected.value(), actual.value()) {
        for (expected, actual) in expected.iter().zip(actual) {
            comparison.origin(expected.origin(), actual.origin())?;
            if let (Some(expected), Some(actual)) = (expected.value(), actual.value()) {
                comparison.origin(
                    expected.declared_block_length().origin(),
                    actual.declared_block_length().origin(),
                )?;
                comparison.origin(expected.raw_word().origin(), actual.raw_word().origin())?;
                comparison.origin(expected.raw_bytes().origin(), actual.raw_bytes().origin())?;
            }
        }
    }
    Ok(())
}
