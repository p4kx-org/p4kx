// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceLocation, SourceUseRole};

use crate::forge_origins::{EvidenceError, ForgeOrigins, site};

use super::super::occurrence_origins as occurrence;
use super::Listing;

pub(super) fn header(
    listing: &Listing<'_, '_, '_>,
    output: &mut ForgeOrigins,
) -> Result<FieldOrigin, EvidenceError> {
    output.put("archive", output.uses.origin(&[], true))?;
    let index = listing.record.index();
    let mut uses = Vec::new();
    for (pointer, field, role) in [
        ("guid", "guid", SourceUseRole::Payload),
        ("record_path", "fileName", SourceUseRole::Conversion),
        ("name", "recordName", SourceUseRole::Payload),
        ("class_name", "recordName", SourceUseRole::Conversion),
    ] {
        let fact = output.scope.at(
            SourceLocation::DataForge {
                record_index: index,
                field: field.into(),
            },
            role,
        )?;
        output.put(
            &format!("entity/{pointer}"),
            output
                .uses
                .origin(std::slice::from_ref(&fact), output.complete),
        )?;
        uses.push(fact);
    }
    output.put("entity", output.uses.origin(&uses, output.complete))?;
    let inspection = site(
        &output.scope,
        "dataforgeRecordTable",
        "canonicalRecord",
        vec![u64::from(index)],
        SourceUseRole::Selector,
    )?;
    let inspection = output
        .uses
        .origin(&[inspection], output.complete && listing.tree_complete);
    let total = listing
        .occurrences
        .iter()
        .map(|row| occurrence::collect(output, row, SourceUseRole::Conversion))
        .collect::<Result<Vec<_>, _>>()?;
    let total = output
        .uses
        .origin_with_origins(std::iter::once(&inspection).chain(&total), true)?;
    output.put("total", total.clone())?;
    Ok(total)
}
