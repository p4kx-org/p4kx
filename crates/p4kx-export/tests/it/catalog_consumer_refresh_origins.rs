// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceInventory, SourcedValue};

use crate::commodity_v3_fixture::TestResult;

pub(super) fn check<T>(
    cell: &SourcedValue<T>,
    inventory: &SourceInventory,
) -> TestResult<Option<bool>> {
    let origin = cell.origin();
    assert!(
        !origin.source_uses_refs().is_empty(),
        "the consumed datum retains its actual source uses",
    );
    for id in origin.source_uses_refs() {
        let usage = inventory
            .iter_uses()
            .find(|row| row.id() == id)
            .ok_or("datum use must belong to this snapshot")?;
        assert!(
            inventory
                .iter_sources()
                .any(|row| row.id() == usage.source()),
            "the physical source belongs to the same snapshot inventory",
        );
    }
    Ok(origin.contains_encrypted_source())
}
