// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::TriggerOccurrence;
use p4kx_extraction::{FieldOrigin, RecordJsonLocation, SourceUseRole};

use crate::forge_origins::{EvidenceError, ForgeOrigins};

pub(super) fn collect(
    output: &ForgeOrigins,
    occurrences: &[TriggerOccurrence],
    value_role: SourceUseRole,
) -> Result<FieldOrigin, EvidenceError> {
    let mut observed = Vec::new();
    let mut complete = output.complete && !occurrences.is_empty();
    for occurrence in occurrences {
        complete &= occurrence.complete();
        for source in occurrence.sources() {
            let Some(location) = RecordJsonLocation::of_use(source) else {
                complete = false;
                continue;
            };
            let role = if location.role == SourceUseRole::Payload {
                value_role
            } else {
                location.role
            };
            observed.push(output.scope.at(location.location, role)?);
        }
    }
    Ok(output.uses.origin(&observed, complete))
}
