// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    SourceLocation, SourceScope, SourceUseRole, SourceUses, SourceValue, SourcedValue, StructValue,
    Value, ValueError,
};

use crate::CatalogError;
use crate::definition_reference::DefinitionStatus;
use crate::guid_text::CatalogGuid;
use crate::resource_float::ResourceFloat32;
use crate::resource_number::ResourceNumber;
use crate::resource_raw_policy::{self, ResourceNumberContract};
use crate::resource_reference::ResourceReference;
use crate::source_fields::{FieldError, Fields};

/// Binds resource fields to the one physical DataForge payload and Record position.
#[derive(Clone, Copy)]
pub(crate) struct ResourceContext<'a> {
    sources: &'a SourceUses,
    dataforge: &'a SourceScope,
    record_index: u32,
}

impl<'a> ResourceContext<'a> {
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
        let (role, complete) = match &value {
            SourceValue::Present(_) | SourceValue::Null | SourceValue::Invalid => {
                (SourceUseRole::Payload, true)
            }
            SourceValue::Absent | SourceValue::Unsupported => (SourceUseRole::Selector, true),
            _ => (SourceUseRole::Attempted, false),
        };
        self.bind(path, value, role, complete)
    }

    pub(crate) fn selector<T>(
        &self,
        path: &str,
        value: SourceValue<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.bind(path, value, SourceUseRole::Selector, true)
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
        let source = self.dataforge.at(self.location(path), role)?;
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

    fn read<'s, 'f, T>(
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

/// Typed field reads before nullable states or source number widths are collapsed.
pub(crate) struct ResourceFields<'s, 'f, 'c> {
    source: &'s StructValue<'f>,
    path: &'s str,
    context: &'s ResourceContext<'c>,
}

impl<'s, 'f, 'c> ResourceFields<'s, 'f, 'c> {
    pub(crate) const fn new(
        source: &'s StructValue<'f>,
        path: &'s str,
        context: &'s ResourceContext<'c>,
    ) -> Self {
        Self {
            source,
            path,
            context,
        }
    }

    pub(crate) fn check(&self, known: &[&str], notes: &mut Vec<FieldError>) {
        Fields::new(self.source, self.path).check(known, notes);
    }

    pub(crate) fn source_type(&self) -> Result<SourcedValue<&'f str>, CatalogError> {
        self.context.selector(
            &self.location("$type"),
            SourceValue::Present(self.source.type_name()),
        )
    }

    pub(crate) fn unsupported<T>(
        &self,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        notes.push(Fields::new(self.source, self.path).unsupported(""));
        self.context.value(self.path, SourceValue::Unsupported)
    }

    pub(crate) fn text(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'f str>, CatalogError> {
        self.read(name, "a nullable String", notes, |value| match value {
            Value::String(value) => Some(*value),
            _ => None,
        })
    }

    pub(crate) fn locale(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'f str>, CatalogError> {
        self.read(name, "a nullable Locale", notes, |value| match value {
            Value::Locale(value) => Some(*value),
            _ => None,
        })
    }

    pub(crate) fn boolean(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<bool>, CatalogError> {
        self.read(name, "a nullable Bool", notes, |value| match value {
            Value::Bool(value) => Some(*value),
            _ => None,
        })
    }

    pub(crate) fn int32(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<i32>, CatalogError> {
        self.read(name, "a nullable Int32", notes, |value| match value {
            Value::Int32(value) => Some(*value),
            _ => None,
        })
    }

    pub(crate) fn required_int32(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<i32>, CatalogError> {
        let value = self.int32(name, notes)?;
        if matches!(self.source.field(name), None | Some(Value::Null)) {
            notes.push(FieldError::Malformed {
                source_type: self.source.type_name().to_owned(),
                field: self.location(name),
                expected: "a non-null Int32",
            });
        }
        Ok(value)
    }

    pub(crate) fn single(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<ResourceFloat32>, CatalogError> {
        self.read(name, "a nullable Single", notes, |value| match value {
            Value::Single(value)
                if resource_raw_policy::field_contract(self.source.type_name(), name)
                    == Some(ResourceNumberContract::Single) =>
            {
                Some(ResourceFloat32::new(*value))
            }
            _ => None,
        })
    }

    pub(crate) fn number(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<ResourceNumber>, CatalogError> {
        self.read(name, "a nullable number", notes, |value| {
            (resource_raw_policy::field_contract(self.source.type_name(), name)
                == Some(ResourceNumberContract::Number))
            .then(|| ResourceNumber::read(value))
            .flatten()
        })
    }

    pub(crate) fn structure(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'s StructValue<'f>>, CatalogError> {
        self.read(
            name,
            "a nullable owned struct",
            notes,
            |value| match value {
                Value::Struct(value) => Some(value),
                _ => None,
            },
        )
    }

    pub(crate) fn array(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'s [Value<'f>]>, CatalogError> {
        self.read(name, "a nullable array", notes, |value| match value {
            Value::Array(value) => Some(value.as_ref()),
            _ => None,
        })
    }

    pub(crate) fn reference(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<ResourceReference>, CatalogError> {
        let path = self.location(name);
        let value = match self.source.field(name) {
            Some(Value::Reference(target)) if target.location().is_none() => {
                Some(ResourceReference::resolved(
                    self.context,
                    &path,
                    target.record_index(),
                    CatalogGuid(target.guid()),
                    target.record_path(),
                    target.type_name(),
                )?)
            }
            Some(Value::Error(ValueError::DanglingReference { guid })) => {
                Some(ResourceReference::unresolved(
                    self.context,
                    &path,
                    CatalogGuid(*guid),
                    DefinitionStatus::Dangling,
                )?)
            }
            Some(Value::Error(ValueError::UnexpectedReferenceType { guid, .. })) => {
                Some(ResourceReference::unresolved(
                    self.context,
                    &path,
                    CatalogGuid(*guid),
                    DefinitionStatus::UnexpectedTargetType,
                )?)
            }
            _ => None,
        };
        value.map_or_else(
            || self.read(name, "a nullable Record Reference", notes, |_| None),
            |value| self.context.value(&path, SourceValue::Present(value)),
        )
    }

    fn read<T>(
        &self,
        name: &str,
        expected: &'static str,
        notes: &mut Vec<FieldError>,
        project: impl FnOnce(&'s Value<'f>) -> Option<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.context.read(
            self.source.field(name),
            self.source.type_name(),
            &self.location(name),
            expected,
            notes,
            project,
        )
    }

    fn location(&self, name: &str) -> String {
        if self.path.is_empty() {
            name.to_owned()
        } else {
            format!("{}.{}", self.path, name)
        }
    }
}
