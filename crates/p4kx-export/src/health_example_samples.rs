// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;

use crate::ExportAccountingError;
use crate::export_diagnostic::EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT;
use crate::health_diagnostic_observation::ExampleObservation;

/// Preserves original frames for equal sample values before selecting deterministic examples.
pub(crate) fn retain(
    mut examples: Vec<ExampleObservation>,
) -> Result<Vec<ExampleObservation>, ExportAccountingError> {
    examples.sort();
    let mut retained: Vec<ExampleObservation> = Vec::new();
    for example in examples {
        if let Some(last) = retained.last_mut()
            && last.sample_cmp(&example) == Ordering::Equal
        {
            last.merge(example)?;
        } else {
            retained.push(example);
        }
    }
    retained.truncate(EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT);
    Ok(retained)
}
