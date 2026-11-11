// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::dataforge::out_of_range;
use crate::rows::{EnumDef, NO_PARENT, PropertyDef, StructDef};
use crate::{DataForge, DataForgeError, DataType};

pub(crate) const CONVERSION_ATTRIBUTE: u16 = 0;

/// A struct definition: a Record type or a type of nested values.
#[derive(Debug, Clone, Copy)]
pub struct Struct<'f> {
    forge: &'f DataForge<'f>,
    index: u32,
    def: StructDef,
}

impl<'f> Struct<'f> {
    /// Returns the name, e.g. `EntityClassDefinition`.
    ///
    /// # Errors
    ///
    /// Returns [`DataForgeError::InvalidString`] when the name offset is not a valid string.
    pub fn name(&self) -> Result<&'f str, DataForgeError> {
        self.forge.blob.get(self.def.name)
    }

    /// Returns the position of the struct in the struct table.
    #[must_use]
    pub const fn index(&self) -> u32 {
        self.index
    }

    /// Returns the struct this one inherits its first fields from, or `None` when it has none.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        (self.def.parent != NO_PARENT)
            .then(|| self.forge.struct_at(self.def.parent))
            .flatten()
    }

    /// Returns the size of one instance in bytes, inherited fields included.
    #[must_use]
    pub const fn size(&self) -> u32 {
        self.def.size
    }

    /// Returns the number of instances the data mappings place.
    #[must_use]
    pub fn instance_count(&self) -> u32 {
        usize::try_from(self.index)
            .ok()
            .and_then(|index| self.forge.instance_counts.get(index))
            .copied()
            .unwrap_or_default()
    }

    /// Returns the properties the struct declares itself, without the inherited ones, in order.
    pub fn properties(&self) -> impl Iterator<Item = Property<'f>> + use<'f> {
        let forge = self.forge;
        let first = usize::from(self.def.first_property);
        (first..first + usize::from(self.def.property_count))
            .filter_map(move |index| forge.properties.get(index))
            .map(move |def| Property { forge, def })
    }
}

/// One field declared by a struct.
#[derive(Debug, Clone, Copy)]
pub struct Property<'f> {
    forge: &'f DataForge<'f>,
    def: PropertyDef,
}

impl<'f> Property<'f> {
    /// Returns the field name.
    ///
    /// # Errors
    ///
    /// Returns [`DataForgeError::InvalidString`] when the name offset is not a valid string.
    pub fn name(&self) -> Result<&'f str, DataForgeError> {
        self.forge.blob.get(self.def.name)
    }

    /// Returns the type of the field's values.
    ///
    /// # Errors
    ///
    /// Returns [`DataForgeError::UnknownDataType`] for a type code this reader does not know.
    pub const fn data_type(&self) -> Result<DataType, DataForgeError> {
        match DataType::from_code(self.def.data_type) {
            Some(data_type) => Ok(data_type),
            None => Err(DataForgeError::UnknownDataType {
                code: self.def.data_type,
            }),
        }
    }

    /// Returns the conversion type: `0` for one value, any other code for an array of values.
    #[must_use]
    pub const fn conversion(&self) -> u16 {
        self.def.conversion
    }

    /// Returns whether the field holds an array of values instead of one.
    #[must_use]
    pub const fn is_array(&self) -> bool {
        self.def.conversion != CONVERSION_ATTRIBUTE
    }

    /// Returns the struct the field's values are instances of, point to or refer to, for class,
    /// pointer and typed Reference fields; `None` for any other field.
    #[must_use]
    pub fn target_struct(&self) -> Option<Struct<'f>> {
        matches!(
            self.data_type(),
            Ok(DataType::Class
                | DataType::StrongPointer
                | DataType::WeakPointer
                | DataType::Reference)
        )
        .then(|| self.forge.struct_at(u32::from(self.def.target)))
        .flatten()
    }

    /// Returns the enum of an enum field; `None` for any other field.
    #[must_use]
    pub fn target_enum(&self) -> Option<Enum<'f>> {
        let forge = self.forge;
        matches!(self.data_type(), Ok(DataType::Enum))
            .then(|| forge.enums.get(usize::from(self.def.target)))
            .flatten()
            .map(|def| Enum { forge, def })
    }
}

/// An enum definition with its option names.
#[derive(Debug, Clone, Copy)]
pub struct Enum<'f> {
    forge: &'f DataForge<'f>,
    def: EnumDef,
}

impl<'f> Enum<'f> {
    /// Returns the name.
    ///
    /// # Errors
    ///
    /// Returns [`DataForgeError::InvalidString`] when the name offset is not a valid string.
    pub fn name(&self) -> Result<&'f str, DataForgeError> {
        self.forge.blob.get(self.def.name)
    }

    /// Returns the option names in order; an option whose name cannot be read is an error.
    pub fn options(
        &self,
    ) -> impl ExactSizeIterator<Item = Result<&'f str, DataForgeError>> + use<'f> {
        let forge = self.forge;
        let first = usize::from(self.def.first_option);
        (first..first + usize::from(self.def.option_count)).map(move |index| {
            let option = forge
                .options
                .get(index)
                .ok_or_else(|| out_of_range("enum option", index, "enum option", index as u64))?;
            forge.blob.get(option.0)
        })
    }
}

impl DataForge<'_> {
    /// Returns every struct definition in order.
    pub fn structs(&self) -> impl ExactSizeIterator<Item = Struct<'_>> {
        self.structs.iter_indexed().map(|(index, def)| Struct {
            forge: self,
            index,
            def,
        })
    }

    /// Returns every enum definition in order.
    pub fn enums(&self) -> impl ExactSizeIterator<Item = Enum<'_>> {
        self.enums.iter().map(|def| Enum { forge: self, def })
    }

    /// Returns the definition at the given physical struct index, when present.
    #[must_use]
    pub fn struct_at(&self, index: u32) -> Option<Struct<'_>> {
        let def = self.structs.get(usize::try_from(index).ok()?)?;
        Some(Struct {
            forge: self,
            index,
            def,
        })
    }
}
