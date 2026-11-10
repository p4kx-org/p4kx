// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use rayon::prelude::*;

use crate::inheritance;
use crate::rows::{NO_PARENT, StructDef};
use crate::slot::Slot;
use crate::{DataForge, DataForgeError, DataType, MAX_RECORD_FIELDS};

/// One field of a struct as a Record tree lists it, with its bytes in an instance.
#[derive(Clone, Copy)]
pub(crate) struct FieldDef<'f> {
    pub(crate) name: &'f str,
    pub(crate) position: u32,
    pub(crate) slot: Slot,
    pub(crate) start: u64,
    pub(crate) end: u64,
}

/// Returns whether values of `data_type` are scalars a column can hold.
pub(crate) const fn is_scalar(data_type: DataType) -> bool {
    !matches!(
        data_type,
        DataType::Class | DataType::StrongPointer | DataType::WeakPointer | DataType::Reference
    )
}

/// The names and fields of every struct, inherited fields first, read once for compiling.
pub(crate) struct StructFields<'f> {
    forge: &'f DataForge<'f>,
    names: Vec<Option<&'f str>>,
    fields: Vec<Result<Vec<FieldDef<'f>>, DataForgeError>>,
}

impl<'f> StructFields<'f> {
    pub(crate) fn read_all(forge: &'f DataForge<'f>) -> Self {
        let rows: Vec<StructDef> = forge.structs.iter().collect();
        let names = rows
            .iter()
            .map(|row| forge.blob.get(row.name).ok())
            .collect();
        let fields = (0..rows.len())
            .into_par_iter()
            .map(|index| fields_of(forge, index))
            .collect();
        Self {
            forge,
            names,
            fields,
        }
    }

    pub(crate) const fn forge(&self) -> &'f DataForge<'f> {
        self.forge
    }

    pub(crate) const fn len(&self) -> usize {
        self.names.len()
    }

    pub(crate) fn names(&self) -> impl Iterator<Item = &'f str> + '_ {
        self.names.iter().flatten().copied()
    }

    pub(crate) fn name(&self, index: usize) -> Option<&'f str> {
        self.names.get(index).copied().flatten()
    }

    pub(crate) fn positions_of<'s>(&'s self, name: &'s str) -> impl Iterator<Item = usize> + 's {
        self.names
            .iter()
            .enumerate()
            .filter(move |(_, candidate)| **candidate == Some(name))
            .map(|(index, _)| index)
    }

    pub(crate) fn position_of(&self, name: &str) -> Option<usize> {
        self.positions_of(name).next()
    }

    pub(crate) fn fields(&self, index: usize) -> Result<&[FieldDef<'f>], &DataForgeError> {
        match self.fields.get(index) {
            Some(Ok(fields)) => Ok(fields),
            Some(Err(error)) => Err(error),
            None => Ok(&[]),
        }
    }

    pub(crate) fn field(&self, index: usize, name: &str) -> Option<FieldDef<'f>> {
        self.fields(index)
            .ok()?
            .iter()
            .find(|field| field.name == name)
            .copied()
    }

    pub(crate) fn field_names(&self, index: usize) -> impl Iterator<Item = &'f str> + '_ {
        self.fields(index)
            .unwrap_or_default()
            .iter()
            .map(|field| field.name)
    }

    pub(crate) fn all_field_names(&self) -> impl Iterator<Item = &'f str> + '_ {
        (0..self.len()).flat_map(|index| self.field_names(index))
    }
}

/// Returns whether the struct at `index` or one of its ancestors is among `targets`.
pub(crate) fn ancestors_include(forge: &DataForge<'_>, index: usize, targets: &[usize]) -> bool {
    let mut current = index;
    for _ in 0..forge.struct_count() {
        if targets.contains(&current) {
            return true;
        }
        match forge.structs.get(current) {
            Some(row) if row.parent != NO_PARENT => {
                let Ok(parent) = usize::try_from(row.parent) else {
                    return false;
                };
                current = parent;
            }
            _ => return false,
        }
    }
    false
}

/// Returns the fields of the struct at `index` in Record tree order, the root ancestor's first,
/// with the layout checks the Record tree makes.
fn fields_of<'f>(
    forge: &'f DataForge<'f>,
    index: usize,
) -> Result<Vec<FieldDef<'f>>, DataForgeError> {
    let struct_index = u32::try_from(index).unwrap_or(u32::MAX);
    let chain = inheritance::chain(forge, struct_index)?;
    let count: usize = chain
        .iter()
        .map(|(_, row)| usize::from(row.property_count))
        .sum();
    if count > MAX_RECORD_FIELDS {
        return Err(DataForgeError::TooManyFields {
            struct_index,
            count,
            limit: MAX_RECORD_FIELDS,
        });
    }
    let mut fields = Vec::with_capacity(count);
    let mut offset = 0_u64;
    for &(owner, row) in chain.iter().rev() {
        let first = usize::from(row.first_property);
        for property in first..first.saturating_add(usize::from(row.property_count)) {
            let slot = Slot::of(forge, owner, property)?;
            let end = offset.saturating_add(slot.size(forge)?);
            fields.push(FieldDef {
                name: slot.name(forge)?,
                position: u32::try_from(fields.len()).unwrap_or(u32::MAX),
                slot,
                start: offset,
                end,
            });
            offset = end;
        }
        if offset != u64::from(row.size) {
            return Err(DataForgeError::StructLayout {
                struct_index: owner,
                declared: row.size,
                fields: offset,
            });
        }
    }
    Ok(fields)
}
