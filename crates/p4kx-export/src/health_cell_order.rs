// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;

use p4kx_extraction::SourcedValue;

use super::health_text_capture::HealthTextCell;

pub(super) fn sourced<T: Ord>(left: &SourcedValue<T>, right: &SourcedValue<T>) -> Ordering {
    compare(left, right, |left, right| left.cmp(&right))
}

pub(super) fn text(left: &HealthTextCell, right: &HealthTextCell) -> Ordering {
    compare(left.cell(), right.cell(), |left, right| {
        left.map(|reference| reference.atom_index())
            .cmp(&right.map(|reference| reference.atom_index()))
    })
    .then_with(|| left.value().cmp(&right.value()))
}

fn compare<T>(
    left: &SourcedValue<T>,
    right: &SourcedValue<T>,
    values: impl FnOnce(Option<&T>, Option<&T>) -> Ordering,
) -> Ordering {
    left.state()
        .cmp(&right.state())
        .then_with(|| values(left.value(), right.value()))
        .then_with(|| {
            left.origin()
                .contains_encrypted_source()
                .cmp(&right.origin().contains_encrypted_source())
        })
        .then_with(|| {
            left.origin()
                .source_uses_refs()
                .cmp(right.origin().source_uses_refs())
        })
        .then_with(|| {
            left.origin()
                .source_set_complete()
                .cmp(&right.origin().source_set_complete())
        })
}
