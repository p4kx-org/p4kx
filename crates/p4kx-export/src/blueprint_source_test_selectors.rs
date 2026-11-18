// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourcedValue};

use crate::catalog_read::original::{OriginalValueFact, OriginalValuePart};

pub(super) fn contains<T>(
    cell: &SourcedValue<T>,
    fact: &OriginalValueFact<'_, '_, '_>,
    part: OriginalValuePart,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = fact.observe(part, SourceUseRole::Selector)?;
    assert!(
        cell.origin()
            .source_uses_refs()
            .contains(actual.reference()),
        "the datum retains the actual original authored selector"
    );
    Ok(())
}
