// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUses, SourceValue, SourcedValue, Value};

use crate::CatalogError;
use crate::catalog_read::original::{OriginalArraySelection, OriginalFlowFields};
use crate::catalog_v3::item_component_owner::ComponentOwner;

pub(super) struct SelectedSource<'read, 'source> {
    pub(super) value: Option<&'read Value<'source>>,
    pub(super) prefix: OriginalArraySelection,
}

impl<'read, 'source> SelectedSource<'read, 'source> {
    pub(super) fn read(
        originals: &OriginalFlowFields<'_, 'read, 'source>,
        array: &str,
        source_type: &str,
    ) -> Result<Self, CatalogError> {
        let value = originals.field(None, array)?.and_then(|(_, fact)| {
            let Value::Array(values) = fact.value() else {
                return None;
            };
            values.iter().find(|value| {
                matches!(value, Value::Struct(source)
                if source.type_name() == source_type)
            })
        });
        Ok(Self {
            value,
            prefix: originals.root_array_selection(array, value)?,
        })
    }

    pub(super) fn ordinal(
        &self,
        originals: &OriginalFlowFields<'_, '_, 'source>,
    ) -> Result<SourcedValue<u32>, CatalogError> {
        let Some(value) = self.value else {
            return Ok(self.missing(originals.sources()));
        };
        let ordinal = ComponentOwner::ordinal(originals, originals.fact(value)?)?;
        self.bind(originals.sources(), ordinal)
    }

    pub(super) fn bind<T>(
        &self,
        sources: &SourceUses,
        value: SourcedValue<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let prefix = sources.origin(&self.prefix.uses, self.prefix.complete);
        Ok(sources.add_value_origins(value, &[&prefix])?)
    }

    pub(super) fn missing<T>(&self, sources: &SourceUses) -> SourcedValue<T> {
        let state = if self.prefix.complete {
            SourceValue::Absent
        } else {
            SourceValue::Unreadable
        };
        sources.value(state, &self.prefix.uses, self.prefix.complete)
    }
}
