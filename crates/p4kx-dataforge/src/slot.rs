// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::dataforge::out_of_range;
use crate::rows::PropertyDef;
use crate::schema::CONVERSION_ATTRIBUTE;
use crate::{DataForge, DataForgeError, DataType};

const ARRAY_LEN: u64 = 8;

/// One property of a struct as it is laid out in an instance: its type and whether it is an
/// array.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Slot {
    property: usize,
    def: PropertyDef,
    data_type: DataType,
    array: bool,
}

impl Slot {
    /// Returns the slot of the property at `property`.
    pub(crate) fn of(
        forge: &DataForge<'_>,
        struct_index: u32,
        property: usize,
    ) -> Result<Self, DataForgeError> {
        let def = forge.properties.get(property).ok_or_else(|| {
            out_of_range("struct", struct_index as usize, "property", property as u64)
        })?;
        let data_type =
            DataType::from_code(def.data_type).ok_or(DataForgeError::UnknownDataType {
                code: def.data_type,
            })?;
        Ok(Self {
            property,
            def,
            data_type,
            array: def.conversion != CONVERSION_ATTRIBUTE,
        })
    }

    pub(crate) const fn property_index(self) -> usize {
        self.property
    }

    /// Returns the type of the field's values.
    pub(crate) const fn data_type(&self) -> DataType {
        self.data_type
    }

    /// Returns the struct or enum the field's values belong to, for a class, a pointer or an
    /// enum.
    pub(crate) const fn target(&self) -> u16 {
        self.def.target
    }

    /// Returns whether the field holds an array.
    pub(crate) const fn is_array(&self) -> bool {
        self.array
    }

    /// Returns the field name.
    pub(crate) fn name<'f>(&self, forge: &DataForge<'f>) -> Result<&'f str, DataForgeError> {
        forge.blob.get(self.def.name)
    }

    /// Returns the number of bytes the field takes in an instance; an inline struct takes the
    /// size of the struct its `target` names.
    pub(crate) fn size(&self, forge: &DataForge<'_>) -> Result<u64, DataForgeError> {
        if self.array {
            return Ok(ARRAY_LEN);
        }
        Ok(match self.data_type {
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
                .get(usize::from(self.def.target))
                .map(|target| u64::from(target.size))
                .ok_or_else(|| {
                    out_of_range("property", self.property, "struct", self.def.target.into())
                })?,
        })
    }
}
