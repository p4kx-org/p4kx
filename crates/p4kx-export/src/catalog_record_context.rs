// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue,
    SourcedValue, StructValue, Value,
};

use crate::CatalogError;
use crate::catalog_number_capture::NumberCapture;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::resource_raw_policy::ResourceNumberContract;
use crate::source_fields::FieldError;

const CARGO_SLOT_NAMESPACE: &str = "dataforgeCargoSlot";

#[derive(Clone, Copy)]
enum BodyCoordinate {
    Record,
    CargoSlot { ordinal: u64 },
}

/// Binds Catalog fields to the one physical DataForge payload and Record position.
#[derive(Clone, Copy)]
pub(crate) struct CatalogRecordContext<'a> {
    sources: &'a SourceUses,
    dataforge: &'a SourceScope,
    record_index: u32,
    body_coordinate: BodyCoordinate,
    numbers: Option<&'a NumberCapture>,
    cancel: Option<&'a CancelToken>,
}

impl<'a> CatalogRecordContext<'a> {
    pub(crate) const fn new(
        sources: &'a SourceUses,
        dataforge: &'a SourceScope,
        record_index: u32,
    ) -> Self {
        Self {
            sources,
            dataforge,
            record_index,
            body_coordinate: BodyCoordinate::Record,
            numbers: None,
            cancel: None,
        }
    }

    pub(crate) const fn sources(&self) -> &'a SourceUses {
        self.sources
    }

    pub(crate) const fn dataforge(&self) -> &'a SourceScope {
        self.dataforge
    }

    pub(crate) const fn record_position(&self) -> Option<u32> {
        match self.body_coordinate {
            BodyCoordinate::Record => Some(self.record_index),
            BodyCoordinate::CargoSlot { .. } => None,
        }
    }

    /// Selects one original field ordinal in this Record's defaultCargoContainers structure.
    pub(crate) const fn cargo_slot(&self, ordinal: u64) -> Self {
        Self {
            body_coordinate: BodyCoordinate::CargoSlot { ordinal },
            ..*self
        }
    }

    /// Binds a local numeric receipt to this original Record context.
    pub(crate) const fn with_numbers(mut self, numbers: &'a NumberCapture) -> Self {
        self.numbers = Some(numbers);
        self
    }

    pub(crate) const fn with_cancel(mut self, cancel: Option<&'a CancelToken>) -> Self {
        self.cancel = cancel;
        self
    }

    pub(crate) fn retain_number(
        &self,
        field: impl FnOnce() -> String,
        source_type: &str,
        contract: ResourceNumberContract,
        value: &Value<'_>,
    ) {
        if let Some(numbers) = self.numbers {
            numbers.retain(self.record_index, field, source_type, contract, value);
        }
    }

    pub(crate) const fn at_record(&self, record_index: u32) -> Self {
        Self {
            record_index,
            body_coordinate: BodyCoordinate::Record,
            numbers: None,
            ..*self
        }
    }

    pub(crate) fn value<T>(
        &self,
        path: &str,
        value: SourceValue<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let (role, complete) = value_policy(&value);
        self.bind(path, value, role, complete)
    }

    pub(crate) fn selector<T>(
        &self,
        path: &str,
        value: SourceValue<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.bind(path, value, SourceUseRole::Selector, true)
    }

    /// Observes an actual operation through this original physical Record context.
    pub(crate) fn observe(
        &self,
        path: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse, CatalogError> {
        Ok(self.dataforge.at(self.location(path), role)?)
    }

    /// Binds a known original header fact without routing an arbitrary body path.
    pub(crate) fn header_value<T>(
        &self,
        field: RecordHeaderField,
        value: SourceValue<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let (role, complete) = value_policy(&value);
        self.bind_at(field.location(self.record_index), value, role, complete)
    }

    pub(crate) fn header_selector<T>(
        &self,
        field: RecordHeaderField,
        value: SourceValue<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.bind_at(
            field.location(self.record_index),
            value,
            SourceUseRole::Selector,
            true,
        )
    }

    pub(crate) fn header_conversion<T>(
        &self,
        field: RecordHeaderField,
        value: SourceValue<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let (_, complete) = value_policy(&value);
        let role = if complete {
            SourceUseRole::Conversion
        } else {
            SourceUseRole::Attempted
        };
        self.bind_at(field.location(self.record_index), value, role, complete)
    }

    pub(crate) fn observe_header(
        &self,
        field: RecordHeaderField,
        role: SourceUseRole,
    ) -> Result<SourceUse, CatalogError> {
        Ok(self.dataforge.at(field.location(self.record_index), role)?)
    }

    /// Retains the actual original filename conversion when a normalized path is consulted.
    pub(crate) fn append_header_input(
        &self,
        input: RecordHeaderInput,
        role: SourceUseRole,
        uses: &mut Vec<SourceUse>,
    ) -> Result<(), CatalogError> {
        match input {
            RecordHeaderInput::Field(field) => uses.push(self.observe_header(field, role)?),
            RecordHeaderInput::NormalizedPath => {
                if role != SourceUseRole::Attempted {
                    uses.push(
                        self.observe_header(
                            RecordHeaderField::FileName,
                            SourceUseRole::Conversion,
                        )?,
                    );
                }
                if role != SourceUseRole::Conversion {
                    uses.push(self.observe_header(RecordHeaderField::FileName, role)?);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn reference_observation(&self, path: &str) -> Result<(), CatalogError> {
        let _observation = self
            .dataforge
            .at(self.location(path), SourceUseRole::Reference)?;
        Ok(())
    }

    fn bind<T>(
        &self,
        path: &str,
        value: SourceValue<T>,
        role: SourceUseRole,
        complete: bool,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.bind_at(self.location(path), value, role, complete)
    }

    fn bind_at<T>(
        &self,
        location: SourceLocation,
        value: SourceValue<T>,
        role: SourceUseRole,
        complete: bool,
    ) -> Result<SourcedValue<T>, CatalogError> {
        if self.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let source = self.dataforge.at(location, role)?;
        Ok(self.sources.value(value, &[source], complete))
    }

    fn location(&self, path: &str) -> SourceLocation {
        match self.body_coordinate {
            BodyCoordinate::Record => SourceLocation::DataForge {
                record_index: self.record_index,
                field: path.to_owned(),
            },
            BodyCoordinate::CargoSlot { ordinal } => SourceLocation::Subresource {
                namespace: CARGO_SLOT_NAMESPACE.to_owned(),
                indices: vec![u64::from(self.record_index), ordinal],
                field: path.to_owned(),
            },
        }
    }

    pub(crate) fn structure_element<'s, 'f>(
        &self,
        value: &'s Value<'f>,
        owner: &str,
        path: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'s StructValue<'f>>, CatalogError> {
        self.read(
            Some(value),
            owner,
            path,
            "a nullable owned struct",
            notes,
            |value| match value {
                Value::Struct(value) => Some(value),
                _ => None,
            },
        )
    }

    pub(crate) fn read<'s, 'f, T>(
        &self,
        value: Option<&'s Value<'f>>,
        owner: &str,
        path: &str,
        expected: &'static str,
        notes: &mut Vec<FieldError>,
        project: impl FnOnce(&'s Value<'f>) -> Option<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.read_with(value, path, notes, project, |value| {
            Ok(match value {
                Value::Error(source) => FieldError::Unreadable {
                    source_type: owner.to_owned(),
                    field: path.to_owned(),
                    source: *source,
                },
                _ => FieldError::Malformed {
                    source_type: owner.to_owned(),
                    field: path.to_owned(),
                    expected,
                },
            })
        })
    }

    pub(super) fn read_with<'s, 'f, T>(
        &self,
        value: Option<&'s Value<'f>>,
        path: &str,
        notes: &mut Vec<FieldError>,
        project: impl FnOnce(&'s Value<'f>) -> Option<T>,
        failure: impl FnOnce(&'s Value<'f>) -> Result<FieldError, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let state = match value {
            None => SourceValue::Absent,
            Some(Value::Null) => SourceValue::Null,
            Some(value @ Value::Error(_)) => {
                notes.push(failure(value)?);
                SourceValue::Unreadable
            }
            Some(value) => {
                if let Some(projected) = project(value) {
                    SourceValue::Present(projected)
                } else {
                    notes.push(failure(value)?);
                    SourceValue::Invalid
                }
            }
        };
        self.value(path, state)
    }
}

const fn value_policy<T>(value: &SourceValue<T>) -> (SourceUseRole, bool) {
    match value {
        SourceValue::Present(_) | SourceValue::Null | SourceValue::Invalid => {
            (SourceUseRole::Payload, true)
        }
        SourceValue::Absent | SourceValue::Unsupported => (SourceUseRole::Selector, true),
        _ => (SourceUseRole::Attempted, false),
    }
}

#[cfg(test)]
#[path = "catalog_record_context_tests.rs"]
mod tests;
