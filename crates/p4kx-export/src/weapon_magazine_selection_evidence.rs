// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    FieldOrigin, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue, Value,
};

use super::{Fields, OriginalValuePart};
use crate::{CatalogError, ExportAccountingError};

#[derive(Default)]
pub(in crate::catalog_v3) struct SelectionEvidence {
    uses: Vec<SourceUse>,
    missing: bool,
}

impl SelectionEvidence {
    pub(in crate::catalog_v3) fn add(&mut self, contribution: SourceUse) {
        self.uses.push(contribution);
    }

    pub(in crate::catalog_v3) fn extend(&mut self, uses: impl IntoIterator<Item = SourceUse>) {
        self.uses.extend(uses);
    }

    pub(in crate::catalog_v3) fn append(&mut self, uses: &mut Vec<SourceUse>) {
        self.uses.append(uses);
    }

    pub(in crate::catalog_v3) fn attempt(&mut self, attempt: SourceUse) {
        self.add(attempt);
        self.missing = true;
    }

    pub(super) fn value<T>(self, sources: &SourceUses, value: T) -> SourcedValue<T> {
        sources.value(SourceValue::Present(value), &self.uses, !self.missing)
    }

    pub(in crate::catalog_v3) fn into_origin(self, sources: &SourceUses) -> FieldOrigin {
        sources.origin(&self.uses, !self.missing)
    }

    pub(in crate::catalog_v3) fn retain(
        &mut self,
        fields: &Fields<'_, '_, '_>,
        selected: Option<&Value<'_>>,
    ) -> Result<bool, CatalogError> {
        let Some((_, root)) = fields.field(None, "Components")? else {
            if selected.is_some() {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            self.add(fields.survey(None)?);
            return Ok(true);
        };
        self.add(root.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
        match root.value() {
            Value::Array(values) => {
                self.add(root.observe(OriginalValuePart::Value, SourceUseRole::Selector)?);
                self.retain_prefix(fields, values, selected)
            }
            _ if selected.is_some() => Err(ExportAccountingError::ConflictingSourceDatum.into()),
            Value::Null => {
                self.add(root.observe(OriginalValuePart::Value, SourceUseRole::Selector)?);
                Ok(true)
            }
            _ => {
                self.attempt(root.observe(OriginalValuePart::Value, SourceUseRole::Attempted)?);
                Ok(false)
            }
        }
    }

    fn retain_prefix(
        &mut self,
        fields: &Fields<'_, '_, '_>,
        values: &[Value<'_>],
        selected: Option<&Value<'_>>,
    ) -> Result<bool, CatalogError> {
        let mut complete = true;
        for value in values {
            let fact = fields.fact(value)?;
            match value {
                Value::Error(_) => {
                    self.attempt(fact.observe(OriginalValuePart::Value, SourceUseRole::Attempted)?);
                    complete = false;
                }
                Value::Struct(_) => self
                    .add(fact.observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector)?),
                _ => self.add(fact.observe(OriginalValuePart::Value, SourceUseRole::Selector)?),
            }
            if selected.is_some_and(|selected| std::ptr::addr_eq(selected, value)) {
                return Ok(complete);
            }
        }
        if selected.is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(complete)
    }
}
