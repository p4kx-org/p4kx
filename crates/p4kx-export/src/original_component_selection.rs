// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, Value};

use super::OriginalFlowFields;
use crate::catalog_read::original::OriginalValuePart;
use crate::{CatalogError, ExportAccountingError};

/// The original root-array declarations consulted up to the chosen occurrence.
pub(crate) struct OriginalArraySelection {
    pub(crate) uses: Vec<SourceUse>,
    pub(crate) complete: bool,
}

impl OriginalFlowFields<'_, '_, '_> {
    /// Observes the Components prefix that admitted the selected value.
    pub(crate) fn component_selection(
        &self,
        selected: Option<&Value<'_>>,
    ) -> Result<OriginalArraySelection, CatalogError> {
        self.root_array_selection("Components", selected)
    }

    /// Observes an actual root-array prefix through its selected original occurrence.
    pub(crate) fn root_array_selection(
        &self,
        name: &str,
        selected: Option<&Value<'_>>,
    ) -> Result<OriginalArraySelection, CatalogError> {
        let mut evidence = OriginalArraySelection {
            uses: Vec::new(),
            complete: true,
        };
        let Some((_, root)) = self.field(None, name)? else {
            if selected.is_some() {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            evidence.uses.push(self.survey(None)?);
            return Ok(evidence);
        };
        evidence
            .uses
            .push(root.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
        match root.value() {
            Value::Array(values) => {
                evidence
                    .uses
                    .push(root.observe(OriginalValuePart::Value, SourceUseRole::Selector)?);
                self.array_prefix(values, selected, &mut evidence)?;
            }
            _ if selected.is_some() => {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            Value::Null => evidence
                .uses
                .push(root.observe(OriginalValuePart::Value, SourceUseRole::Selector)?),
            _ => {
                evidence.complete = false;
                evidence
                    .uses
                    .push(root.observe(OriginalValuePart::Value, SourceUseRole::Attempted)?);
            }
        }
        Ok(evidence)
    }

    fn array_prefix(
        &self,
        values: &[Value<'_>],
        selected: Option<&Value<'_>>,
        evidence: &mut OriginalArraySelection,
    ) -> Result<(), CatalogError> {
        for value in values {
            let fact = self.fact(value)?;
            let (part, role) = match value {
                Value::Error(_) => {
                    evidence.complete = false;
                    (OriginalValuePart::Value, SourceUseRole::Attempted)
                }
                Value::Struct(_) => (OriginalValuePart::ConcreteType, SourceUseRole::Selector),
                _ => (OriginalValuePart::Value, SourceUseRole::Selector),
            };
            evidence.uses.push(fact.observe(part, role)?);
            if selected.is_some_and(|selected| std::ptr::addr_eq(selected, value)) {
                return Ok(());
            }
        }
        if selected.is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(())
    }
}
