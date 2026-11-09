// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
    StructValue, Value,
};

use crate::CatalogError;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::source_fields::FieldError;

/// Binds Catalog fields to the one physical DataForge payload and Record position.
#[derive(Clone, Copy)]
pub(crate) struct CatalogRecordContext<'a> {
    sources: &'a SourceUses,
    dataforge: &'a SourceScope,
    record_index: u32,
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
        }
    }

    pub(crate) const fn at_record(&self, record_index: u32) -> Self {
        Self {
            record_index,
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
        let source = self.dataforge.at(location, role)?;
        Ok(self.sources.value(value, &[source], complete))
    }

    fn location(&self, path: &str) -> SourceLocation {
        SourceLocation::DataForge {
            record_index: self.record_index,
            field: path.to_owned(),
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
        let state = match value {
            None => SourceValue::Absent,
            Some(Value::Null) => SourceValue::Null,
            Some(Value::Error(source)) => {
                notes.push(FieldError::Unreadable {
                    source_type: owner.to_owned(),
                    field: path.to_owned(),
                    source: *source,
                });
                SourceValue::Unreadable
            }
            Some(value) => project(value).map_or_else(
                || {
                    notes.push(FieldError::Malformed {
                        source_type: owner.to_owned(),
                        field: path.to_owned(),
                        expected,
                    });
                    SourceValue::Invalid
                },
                SourceValue::Present,
            ),
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
