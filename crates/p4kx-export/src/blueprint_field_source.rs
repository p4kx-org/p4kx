// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourceUseRole, SourceValue, SourcedValue, StructValue, Value};

use crate::CatalogError;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::source_value_catalog::SourceValueRef;

pub(super) struct BlueprintField<'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    owner: Option<&'fields OriginalValueFact<'scope, 'read, 'source>>,
    value: Option<&'fields OriginalValueFact<'scope, 'read, 'source>>,
    blocked: bool,
}

impl<'fields, 'scope, 'read, 'source> BlueprintField<'fields, 'scope, 'read, 'source> {
    pub(super) fn read(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        source: &StructValue<'source>,
        name: &str,
    ) -> Result<Self, CatalogError> {
        let owner = fields.owner_of(source)?;
        let value = fields.field(owner, name)?.map(|(_, fact)| fact);
        Ok(Self {
            fields,
            owner,
            value,
            blocked: false,
        })
    }

    pub(super) fn child(&self, name: &str) -> Result<Self, CatalogError> {
        let Some(fact) = self.value else {
            return Ok(Self {
                fields: self.fields,
                owner: self.owner,
                value: None,
                blocked: true,
            });
        };
        match fact.value() {
            Value::Struct(source) => Self::read(self.fields, source, name),
            _ => Ok(Self {
                fields: self.fields,
                owner: self.owner,
                value: Some(fact),
                blocked: true,
            }),
        }
    }

    pub(super) const fn fact(&self) -> Option<&'fields OriginalValueFact<'scope, 'read, 'source>> {
        self.value
    }

    pub(super) const fn is_blocked(&self) -> bool {
        self.blocked
    }

    pub(super) fn reference(&self) -> Option<SourceValueRef> {
        if self.blocked {
            None
        } else {
            self.value.map(value_ref)
        }
    }

    pub(super) fn derived<T>(
        &self,
        convert: impl FnOnce(&Value<'source>) -> SourceValue<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.value.map_or_else(
            || self.absent(),
            |fact| {
                let value = if self.blocked {
                    normalize(fact.value(), None)
                } else {
                    convert(fact.value())
                };
                self.bind(fact, value, SourceUseRole::Conversion)
            },
        )
    }

    pub(super) fn selected<T>(
        &self,
        value: SourceValue<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.value.map_or_else(
            || self.absent(),
            |fact| self.bind(fact, value, SourceUseRole::Selector),
        )
    }

    pub(super) fn absence_input(&self) -> Result<p4kx_extraction::SourceUse, CatalogError> {
        self.fields.survey(self.owner)
    }

    pub(super) fn absent<T>(&self) -> Result<SourcedValue<T>, CatalogError> {
        let source = self.absence_input()?;
        Ok(self
            .fields
            .sources()
            .value(SourceValue::Absent, &[source], true))
    }

    fn bind<T>(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        value: SourceValue<T>,
        role: SourceUseRole,
    ) -> Result<SourcedValue<T>, CatalogError> {
        Ok(fact
            .bind(OriginalValuePart::Value, value, role, true)?
            .into_cell(self.fields.sources()))
    }
}

pub(super) const fn value_ref(fact: &OriginalValueFact<'_, '_, '_>) -> SourceValueRef {
    SourceValueRef {
        value_index: DecimalU64::new(fact.ordinal()),
    }
}

pub(super) fn normalize<T>(source: &Value<'_>, value: Option<T>) -> SourceValue<T> {
    match source {
        Value::Null => SourceValue::Null,
        Value::Error(_) => SourceValue::Unreadable,
        _ => value.map_or(SourceValue::Invalid, SourceValue::Present),
    }
}
