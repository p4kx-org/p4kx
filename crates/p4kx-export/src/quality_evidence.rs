// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue, StructValue, Value};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::CatalogRecordContext;
use crate::record_header_field::RecordHeaderInput;

/// Actual original decision inputs and the completeness of the consulted source.
#[derive(Debug)]
pub(crate) struct Evidence {
    uses: Vec<SourceUse>,
    pub(crate) complete: bool,
}

impl Default for Evidence {
    fn default() -> Self {
        Self {
            uses: Vec::new(),
            complete: true,
        }
    }
}

impl Evidence {
    pub(crate) fn header(
        &mut self,
        context: &CatalogRecordContext<'_>,
        field: RecordHeaderInput,
        readable: bool,
    ) -> Result<(), CatalogError> {
        self.uses
            .try_reserve(2)
            .map_err(CatalogError::SourceUseAllocation)?;
        context.append_header_input(
            field,
            if readable {
                SourceUseRole::Selector
            } else {
                SourceUseRole::Attempted
            },
            &mut self.uses,
        )?;
        self.complete &= readable;
        Ok(())
    }

    pub(super) fn lookup<'view, 'source>(
        &mut self,
        context: &CatalogRecordContext<'_>,
        parent: &'view StructValue<'source>,
        name: &str,
        path: &str,
        survey: impl FnOnce() -> Result<SourceUse, CatalogError>,
    ) -> Result<Option<&'view Value<'source>>, CatalogError> {
        let value = parent.field(name);
        if let Some(value) = value {
            self.field(context, path, value)?;
        } else {
            self.keep(survey()?)?;
        }
        Ok(value)
    }

    pub(super) fn field(
        &mut self,
        context: &CatalogRecordContext<'_>,
        path: &str,
        value: &Value<'_>,
    ) -> Result<(), CatalogError> {
        let readable = !matches!(value, Value::Error(_));
        let role = if readable {
            SourceUseRole::Selector
        } else {
            SourceUseRole::Attempted
        };
        self.keep(context.observe(path, role)?)?;
        self.complete &= readable;
        Ok(())
    }

    pub(super) fn concrete_type<'source>(
        &mut self,
        context: &CatalogRecordContext<'_>,
        path: &str,
        value: &StructValue<'source>,
    ) -> Result<&'source str, CatalogError> {
        self.keep(context.observe(&format!("{path}.$type"), SourceUseRole::Selector)?)?;
        Ok(value.type_name())
    }

    pub(crate) fn keep(&mut self, source: SourceUse) -> Result<(), CatalogError> {
        self.uses
            .try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        self.uses.push(source);
        Ok(())
    }

    pub(crate) fn append(&mut self, other: &Self) -> Result<(), CatalogError> {
        self.uses
            .try_reserve(other.uses.len())
            .map_err(CatalogError::SourceUseAllocation)?;
        self.uses.extend(other.uses.iter().cloned());
        self.complete &= other.complete;
        Ok(())
    }

    pub(super) fn clear(&mut self) {
        self.uses.clear();
        self.complete = true;
    }

    pub(crate) fn copy(&self) -> Result<Self, CatalogError> {
        let mut result = Self::default();
        result.append(self)?;
        Ok(result)
    }

    pub(crate) const fn inputs(&self) -> &[SourceUse] {
        self.uses.as_slice()
    }

    pub(crate) fn value<T>(&self, origins: &CatalogOrigins, value: T) -> SourcedValue<T> {
        origins
            .uses
            .value(SourceValue::Present(value), self.inputs(), self.complete)
    }
}
