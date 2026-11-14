// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};

use crate::catalog_v3::interaction_source::RecordLink;

/// Gives the projection state used to retain an original array with a rejected slot.
pub(in crate::catalog_v3) fn complete_array_state<T>(
    array: &SourcedValue<Vec<SourcedValue<T>>>,
) -> SourceValueState {
    if let Some(slots) = array.value()
        && let Some(rejected) = slots
            .iter()
            .map(SourcedValue::state)
            .find(|state| !matches!(state, SourceValueState::Present | SourceValueState::Null))
    {
        return rejected;
    }
    array.state()
}

/// Returns the physical Record index of a resolved Reference with compatible declaration.
pub(in crate::catalog_v3) fn linked_record_index(link: &RecordLink<'_>) -> Option<u32> {
    link.value()?.resolved_index()
}
