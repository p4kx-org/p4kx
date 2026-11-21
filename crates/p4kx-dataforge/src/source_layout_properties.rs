// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::rows::StructDef;
use crate::slot::Slot;
use crate::source_layout_allocation;
use crate::{DataForge, DataForgeError};

/// One successfully decoded physical property declaration.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceField {
    pub(crate) slot: Slot,
    pub(crate) width: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct Boundary {
    fields: usize,
    bytes: u64,
}

/// Decoded physical properties and prefix coordinates, without inherited copies.
#[derive(Debug)]
pub(crate) struct SourceProperties {
    pub(crate) fields: Box<[SourceField]>,
    boundaries: Box<[Boundary]>,
    failures: Box<[(usize, DataForgeError)]>,
}

impl SourceProperties {
    pub(crate) fn read(forge: &DataForge<'_>, rows: &[StructDef]) -> Result<Self, DataForgeError> {
        let end = rows
            .iter()
            .filter(|row| row.property_count != 0)
            .map(|row| usize::from(row.first_property) + usize::from(row.property_count))
            .max()
            .unwrap_or_default();
        let mut fields = Vec::new();
        let mut boundaries = Vec::new();
        let mut failures = Vec::new();
        source_layout_allocation::reserve(&mut fields, end, "source layout properties")?;
        source_layout_allocation::reserve(&mut boundaries, end + 1, "source layout coordinates")?;
        source_layout_allocation::reserve(&mut failures, end, "source layout errors")?;
        let mut boundary = Boundary::default();
        boundaries.push(boundary);
        for property in 0..end {
            match read(forge, property) {
                Ok(field) => {
                    boundary.bytes = boundary.bytes.checked_add(field.width).ok_or(
                        DataForgeError::TooLarge {
                            part: "source property widths",
                        },
                    )?;
                    fields.push(field);
                    boundary.fields = fields.len();
                }
                Err(error) => failures.push((property, error)),
            }
            boundaries.push(boundary);
        }
        Ok(Self {
            fields: fields.into_boxed_slice(),
            boundaries: boundaries.into_boxed_slice(),
            failures: failures.into_boxed_slice(),
        })
    }

    pub(crate) fn range(&self, row: StructDef) -> Result<(Range<usize>, u64), DataForgeError> {
        if row.property_count == 0 {
            return Ok((0..0, 0));
        }
        let first = usize::from(row.first_property);
        let end = first + usize::from(row.property_count);
        let failure = self
            .failures
            .partition_point(|(property, _)| *property < first);
        if let Some((property, error)) = self.failures.get(failure)
            && *property < end
        {
            return Err(error.clone());
        }
        let first = self
            .boundaries
            .get(first)
            .ok_or(DataForgeError::LayoutInvariant)?;
        let end = self
            .boundaries
            .get(end)
            .ok_or(DataForgeError::LayoutInvariant)?;
        let bytes = end
            .bytes
            .checked_sub(first.bytes)
            .ok_or(DataForgeError::LayoutInvariant)?;
        Ok((first.fields..end.fields, bytes))
    }
}

fn read(forge: &DataForge<'_>, property: usize) -> Result<SourceField, DataForgeError> {
    let slot = Slot::of(forge, 0, property)?;
    let width = slot.size(forge)?;
    let _name = slot.name(forge)?;
    Ok(SourceField { slot, width })
}
