// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::paths;
use crate::resolver_observation::Observation;
use crate::{LoadoutError, LoadoutPath, LoadoutPathOperation};

pub(super) fn add(
    candidates: &mut Vec<LoadoutPath>,
    value: &str,
    operation: LoadoutPathOperation,
    inputs: &[&LoadoutPath],
    observation: Option<&Observation<'_>>,
) -> Result<(), LoadoutError> {
    if let Some(value) = paths::confine(value) {
        push(candidates, value, operation, inputs, observation)?;
    }
    Ok(())
}

pub(super) fn push(
    candidates: &mut Vec<LoadoutPath>,
    value: String,
    operation: LoadoutPathOperation,
    inputs: &[&LoadoutPath],
    observation: Option<&Observation<'_>>,
) -> Result<(), LoadoutError> {
    if candidates
        .iter()
        .any(|seen| seen.eq_ignore_ascii_case(&value))
    {
        return Ok(());
    }
    let path = LoadoutPath::derived(value, operation, inputs, observation)?;
    candidates.push(path);
    Ok(())
}
