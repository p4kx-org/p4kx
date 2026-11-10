// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

use crate::dataforge::out_of_range;
use crate::inheritance;
use crate::schema::CONVERSION_ATTRIBUTE;
use crate::{DataForge, DataForgeError, DataType};

/// The most fields one struct may have, inherited ones included.
pub const MAX_RECORD_FIELDS: usize = 1 << 16;

const ARRAY_LEN: u64 = 8;
pub(crate) const CACHE_FIELDS: usize = 1 << 20;

/// One field of a struct as it lies in an instance.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FieldSlot<'f> {
    pub(crate) name: &'f str,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) data_type: DataType,
    pub(crate) array: bool,
    pub(crate) target: u16,
}

/// The name and the fields of a struct, inherited ones first, checked against its size.
#[derive(Debug, Clone)]
pub(crate) struct Layout<'f> {
    pub(crate) type_name: &'f str,
    pub(crate) fields: Arc<[FieldSlot<'f>]>,
}

impl<'f> Layout<'f> {
    /// Reads the layout of the struct at `struct_index`, which must exist.
    pub(crate) fn of(forge: &'f DataForge<'f>, struct_index: u32) -> Result<Self, DataForgeError> {
        let chain = inheritance::chain(forge, struct_index)?;
        let count: usize = chain
            .iter()
            .map(|(_, def)| usize::from(def.property_count))
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
        for &(index, structure) in chain.iter().rev() {
            let first = usize::from(structure.first_property);
            for property in first..first + usize::from(structure.property_count) {
                let slot = slot(forge, index, property, offset)?;
                offset = slot.1;
                fields.push(slot.0);
            }
            if offset != u64::from(structure.size) {
                return Err(DataForgeError::StructLayout {
                    struct_index: index,
                    declared: structure.size,
                    fields: offset,
                });
            }
        }
        let record = chain.first().map(|(_, def)| def.name).unwrap_or_default();
        Ok(Self {
            type_name: forge.blob.get(record)?,
            fields: fields.into(),
        })
    }
}

/// Returns the slot of the property at `property` of the struct at `struct_index`, starting at
/// `start`, and where it ends.
fn slot<'f>(
    forge: &'f DataForge<'f>,
    struct_index: u32,
    property: usize,
    start: u64,
) -> Result<(FieldSlot<'f>, u64), DataForgeError> {
    let def = forge.properties.get(property).ok_or_else(|| {
        out_of_range("struct", struct_index as usize, "property", property as u64)
    })?;
    let data_type = DataType::from_code(def.data_type).ok_or(DataForgeError::UnknownDataType {
        code: def.data_type,
    })?;
    let array = def.conversion != CONVERSION_ATTRIBUTE;
    let size = if array {
        ARRAY_LEN
    } else {
        match data_type {
            DataType::Bool | DataType::Int8 | DataType::UInt8 => 1,
            DataType::Int16 | DataType::UInt16 => 2,
            DataType::Int32
            | DataType::UInt32
            | DataType::Single
            | DataType::String
            | DataType::Locale
            | DataType::Enum => 4,
            DataType::Int64
            | DataType::UInt64
            | DataType::Double
            | DataType::StrongPointer
            | DataType::WeakPointer => 8,
            DataType::Guid => 16,
            DataType::Reference => 20,
            DataType::Class => forge
                .structs
                .get(usize::from(def.target))
                .map(|target| u64::from(target.size))
                .ok_or_else(|| out_of_range("property", property, "struct", def.target.into()))?,
        }
    };
    let end = start.saturating_add(size);
    let too_large = || DataForgeError::TooLarge { part: "struct" };
    let field = FieldSlot {
        name: forge.blob.get(def.name)?,
        start: usize::try_from(start).map_err(|_| too_large())?,
        end: usize::try_from(end).map_err(|_| too_large())?,
        data_type,
        array,
        target: def.target,
    };
    Ok((field, end))
}

/// The layouts read so far, kept until they hold more than a fixed number of fields.
#[derive(Debug, Default)]
pub(crate) struct Layouts<'f> {
    cached: HashMap<u32, Layout<'f>>,
    fields: usize,
}

impl<'f> Layouts<'f> {
    /// Returns the layout of the struct at `struct_index`, which must exist.
    pub(crate) fn get(
        &mut self,
        forge: &'f DataForge<'f>,
        struct_index: u32,
    ) -> Result<Layout<'f>, DataForgeError> {
        if let Some(layout) = self.cached.get(&struct_index) {
            return Ok(layout.clone());
        }
        let layout = Layout::of(forge, struct_index)?;
        if self.fields + layout.fields.len() > CACHE_FIELDS {
            self.clear();
        }
        self.fields += layout.fields.len();
        self.cached.insert(struct_index, layout.clone());
        Ok(layout)
    }

    pub(crate) fn clear(&mut self) {
        self.cached.clear();
        self.fields = 0;
    }
}
