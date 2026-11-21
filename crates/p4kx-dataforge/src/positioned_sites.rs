// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::positioned_record::ValueSlot;
use crate::positioned_reference_members::DanglingReferenceMember;
use crate::{
    DataForgeError, DataType, Field, PositionedRecord, PositionedReferenceSources,
    PositionedSources, RecordJsonBytes, RecordJsonReferenceSources, RecordJsonSource, StructValue,
    Value, ValueError,
};

/// The canonical fields and original slots of one admitted structure.
pub struct PositionedFields<'record, 'source> {
    record: &'record PositionedRecord<'source>,
    fields: &'record [Field<'source>],
    slots: Vec<ValueSlot<'source>>,
}

impl<'source> PositionedRecord<'source> {
    /// Positions the root's fields in inherited-first declaration order.
    ///
    /// # Errors
    /// Returns a source-layout or slot-allocation error.
    pub fn root_fields(&self) -> Result<PositionedFields<'_, 'source>, DataForgeError> {
        PositionedFields::new(self, self.tree().root())
    }
}

impl<'record, 'source> PositionedFields<'record, 'source> {
    fn new(
        record: &'record PositionedRecord<'source>,
        structure: &'record StructValue<'source>,
    ) -> Result<Self, DataForgeError> {
        let fields = structure.fields();
        let slots = record.fields(structure.struct_index(), fields)?;
        if slots.len() != fields.len() {
            return Err(DataForgeError::CanonicalTreeInvariant);
        }
        Ok(Self {
            record,
            fields,
            slots,
        })
    }

    /// Borrows the field with this original inherited-first ordinal.
    #[must_use]
    pub fn get(&self, ordinal: usize) -> Option<PositionedValue<'record, 'source>> {
        let field = self.fields.get(ordinal)?;
        let slot = *self.slots.get(ordinal)?;
        Some(PositionedValue {
            record: self.record,
            value: field.value(),
            name: Some(field.name()),
            slot,
        })
    }
}

/// One canonical value paired with its admitted original field or element slot.
#[derive(Clone, Copy)]
pub struct PositionedValue<'record, 'source> {
    record: &'record PositionedRecord<'source>,
    value: &'record Value<'source>,
    name: Option<&'source str>,
    slot: ValueSlot<'source>,
}

impl<'record, 'source> PositionedValue<'record, 'source> {
    /// Returns the canonical field or element value.
    #[must_use]
    pub const fn value(&self) -> &'record Value<'source> {
        self.value
    }

    /// Returns the storage declaration's element data type.
    #[must_use]
    pub const fn data_type(&self) -> DataType {
        self.slot.data_type
    }

    /// Returns the original property name for a field.
    #[must_use]
    pub const fn field_name(&self) -> Option<&'source str> {
        self.name
    }

    /// Returns the original property declaration and selected name bytes for a field.
    #[must_use]
    pub fn field_name_sources(&self) -> Option<PositionedSources> {
        let name = self.field_name()?;
        let mut sources = PositionedSources::new(
            vec![RecordJsonSource::PropertyName {
                index: self.slot.property,
            }],
            true,
        );
        sources.require(self.record.describe_text(name));
        Some(sources)
    }

    /// Returns the local declaration, array header and storage bytes used to decode the value.
    ///
    /// This scope covers storage only, including slot bytes from a failed decode.
    #[must_use]
    pub fn storage_sources(&self) -> PositionedSources {
        if self.slot.data_type == DataType::Reference
            && (!self.slot.array || self.slot.array_header.is_some())
        {
            return PositionedReferenceSources::caller_storage(self.record, self.slot);
        }
        let mut sources = self.record.describe_storage(self.slot);
        if self.slot.bytes == Some(&[]) && !matches!(self.value, Value::Struct(_)) {
            sources.incomplete();
        }
        sources
    }

    /// Returns the local declaration and selectors used to enter a structure or array.
    ///
    /// An inline class is selected by its declaration and any array header.
    #[must_use]
    pub fn selection_sources(&self) -> Option<PositionedSources> {
        self.record
            .describe_container_selection(self.slot, self.value)
    }

    /// Returns local storage and data used for a scalar, null, array length or structure name.
    ///
    /// Callers retain ancestor selection separately. Links and errors carry partial observations.
    #[must_use]
    pub fn value_sources(&self) -> PositionedSources {
        let mut sources = self
            .selection_sources()
            .unwrap_or_else(|| self.storage_sources());
        match self.value {
            Value::String(text) | Value::Locale(text) | Value::Enum(text) => {
                sources.require(self.record.describe_text(text));
            }
            Value::Struct(structure) => sources.push(RecordJsonSource::StructName {
                index: structure.struct_index(),
            }),
            Value::Reference(_) | Value::Error(_) => sources.incomplete(),
            _ => {}
        }
        sources
    }

    /// Returns the caller and selected-target facts of an admitted link.
    #[must_use]
    pub fn reference_sources(&self) -> Option<PositionedReferenceSources> {
        let Value::Reference(reference) = self.value else {
            return None;
        };
        PositionedReferenceSources::capture(self.record, self.slot, reference)
    }

    /// Returns the original contributors to a dangling Reference error.
    #[must_use]
    pub fn dangling_reference_error_sources(&self) -> Option<RecordJsonReferenceSources> {
        match self.value {
            Value::Error(ValueError::DanglingReference { .. })
                if self.slot.data_type == DataType::Reference =>
            {
                Some(RecordJsonReferenceSources::dangling(
                    self.record,
                    Some(self.slot),
                    DanglingReferenceMember::Error,
                ))
            }
            _ => None,
        }
    }

    /// Returns the actual array count and first element words for an array or its element.
    #[must_use]
    pub fn array_header_source(&self) -> Option<RecordJsonSource<'static>> {
        let header = if matches!(self.value, Value::Array(_)) {
            self.slot.bytes
        } else {
            self.slot.array_header
        };
        self.record
            .describe_bytes(header, RecordJsonBytes::ArrayHeader)
    }

    /// Borrows an admitted element at its original array ordinal.
    #[must_use]
    pub fn element(&self, ordinal: u32) -> Option<Self> {
        let Value::Array(elements) = self.value else {
            return None;
        };
        let element = elements.get(usize::try_from(ordinal).ok()?)?;
        Some(Self {
            record: self.record,
            value: element,
            name: None,
            slot: self.record.array_element(self.slot, ordinal),
        })
    }

    /// Positions the fields of an admitted owned structure.
    ///
    /// # Errors
    /// Returns a source-layout or slot-allocation error.
    pub fn fields(&self) -> Result<Option<PositionedFields<'record, 'source>>, DataForgeError> {
        let Value::Struct(structure) = self.value else {
            return Ok(None);
        };
        PositionedFields::new(self.record, structure).map(Some)
    }
}
