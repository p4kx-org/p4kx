// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::catalog_number_capture::{CapturedNumber, FloatToken, NumberCapture, NumberOccurrence};
use crate::resource_raw_policy::ResourceNumberContract;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn repeated_views_share_one_identical_physical_numeric_receipt() -> TestResult {
    let records = NumberCapture::deduplicate(vec![
        number(7, 3, "same", 0x7fc0_1166),
        number(7, 3, "same", 0x7fc0_1166),
        number(7, 4, "same", 0x7fc0_1166),
        number(8, 3, "same", 0x7fc0_1166),
    ])?;
    assert_eq!(
        records.len(),
        3,
        "only duplicate views of the same original occurrence merge"
    );
    Ok(())
}

#[test]
fn contradictory_bits_at_one_physical_numeric_occurrence_are_rejected() {
    let result = NumberCapture::deduplicate(vec![
        number(7, 3, "same", 0x7fc0_1166),
        number(7, 3, "same", 0xffc0_1166),
    ]);
    assert!(
        matches!(
            result,
            Err(crate::CatalogError::Accounting(
                crate::ExportAccountingError::ConflictingSourceDatum
            ))
        ),
        "a capture conflict is never solved by taking the first view"
    );
}

fn number(record_index: u32, ordinal: u64, field: &str, bits: u32) -> CapturedNumber {
    CapturedNumber {
        record_index,
        occurrence: NumberOccurrence::AuthoredDefinitionRawValue { ordinal },
        field: field.to_owned(),
        token: FloatToken::Single(bits),
        source_type: "ObservedDefinition".to_owned(),
        contract: ResourceNumberContract::Number,
    }
}
