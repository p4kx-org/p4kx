// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::json;

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{Input, TestResult};
use crate::health_reference_assertions as references;
use crate::health_reference_fixture as fixture;
use crate::health_reference_source as source;

#[test]
fn health_reference_datums_keep_original_cells_and_precise_caller_target_contributions()
-> TestResult {
    let bytes = fixture::database();
    source::original(&bytes)?;
    for input in [Input::Plain, Input::Encrypted] {
        let catalog = fixture::document(&bytes, input)?;
        let rows = references::rows(&catalog)?;
        assert_eq!(
            rows.len(),
            33,
            "all distinct original Reference occurrences survive without sampling"
        );
        references::resolved(
            &catalog,
            rows.first().ok_or("first actual Reference")?,
            "Left[0]",
            2,
            input.marker(),
        )?;
        references::resolved(
            &catalog,
            rows.last().ok_or("last actual Reference")?,
            "Right[15]",
            35,
            input.marker(),
        )?;
        check::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn health_reference_repeated_long_text_is_shared_while_occurrence_origins_stay_distinct()
-> TestResult {
    let bytes = fixture::database();
    source::original(&bytes)?;
    let catalog = fixture::document(&bytes, Input::Encrypted)?;
    let rows = references::rows(&catalog)?;
    assert_eq!(
        rows.len(),
        33,
        "all original equal-target slots remain physical occurrences"
    );
    let mut target_atoms = std::collections::BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        let (field, ordinal) = if index < fixture::LEFT {
            (format!("Left[{index}]"), 2 + u64::try_from(index)?)
        } else {
            let index = index - fixture::LEFT;
            (format!("Right[{index}]"), 20 + u64::try_from(index)?)
        };
        references::resolved(&catalog, row, &field, ordinal, Some(true))?;
        target_atoms.insert(
            check::at(row, "/targetRecordPath/value/atomIndex")?
                .as_str()
                .ok_or("actual target atom")?,
        );
    }
    assert_eq!(
        target_atoms.len(),
        1,
        "one exact long spelling is stored once without merging occurrences"
    );
    let texts = check::rows(&catalog, "/exportHealth/metadata/texts")?;
    assert_eq!(
        texts
            .iter()
            .filter(|text| text.get("value") == Some(&json!(fixture::target_path())))
            .count(),
        1,
        "the final retained store has exactly one actual target spelling"
    );
    let first = rows.first().ok_or("first actual Reference")?;
    let last = rows.last().ok_or("last actual Reference")?;
    assert_ne!(
        check::at(first, "/targetRecordPath/sourceUsesRefs")?,
        check::at(last, "/targetRecordPath/sourceUsesRefs")?,
        "shared text does not replace independent source contributions"
    );
    check::closure(&catalog)
}
