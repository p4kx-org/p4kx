// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceUses, SourceValue, Value};

use crate::catalog_read::original::{OriginalValueFact, OriginalValuePart};
use crate::contract_facts::{FactLayer, Stated};
use crate::{CatalogError, ExportAccountingError};

#[derive(Default)]
pub(super) struct Evidence {
    selectors: Vec<SourceUse>,
    attempted: bool,
}

impl Evidence {
    pub(super) fn consult(
        &mut self,
        fact: &OriginalValueFact<'_, '_, '_>,
    ) -> Result<(), CatalogError> {
        let role = if matches!(fact.value(), Value::Error(_)) {
            SourceUseRole::Attempted
        } else {
            SourceUseRole::Selector
        };
        self.retain(fact.observe(OriginalValuePart::Value, role)?)
    }

    pub(super) fn retain(&mut self, used: SourceUse) -> Result<(), CatalogError> {
        if used.role() == SourceUseRole::Attempted {
            self.attempted = true;
            return Ok(());
        }
        if used.role() != SourceUseRole::Selector {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.selectors
            .try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        self.selectors.push(used);
        Ok(())
    }

    pub(super) fn extend(&mut self, other: &Self) -> Result<(), CatalogError> {
        extend(&mut self.selectors, &other.selectors)?;
        self.attempted |= other.attempted;
        Ok(())
    }
}

pub(crate) fn extend(into: &mut Vec<SourceUse>, inputs: &[SourceUse]) -> Result<(), CatalogError> {
    into.try_reserve(inputs.len())
        .map_err(CatalogError::SourceUseAllocation)?;
    into.extend_from_slice(inputs);
    Ok(())
}

#[derive(Clone)]
pub(super) struct Selected<T> {
    value: T,
    layer: FactLayer,
    datum: SourceUse,
}

impl<T> Selected<T> {
    pub(super) fn read(
        value: T,
        layer: FactLayer,
        fact: &OriginalValueFact<'_, '_, '_>,
        role: SourceUseRole,
    ) -> Result<Self, CatalogError> {
        let datum = fact.observe(OriginalValuePart::Value, role)?;
        Ok(Self {
            value,
            layer,
            datum,
        })
    }
}

impl Selected<bool> {
    pub(super) fn template(value: bool, datum: SourceUse) -> Result<Self, CatalogError> {
        if datum.role() != SourceUseRole::Payload {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(Self {
            value,
            layer: FactLayer::Template,
            datum,
        })
    }
}

pub(crate) struct Choice<T> {
    selected: Option<Selected<T>>,
    evidence: Evidence,
}

impl<T: Copy> Choice<T> {
    pub(super) const fn new(selected: Option<Selected<T>>, evidence: Evidence) -> Self {
        Self { selected, evidence }
    }

    pub(super) fn inherit(
        selected: Option<Selected<T>>,
        mut evidence: Evidence,
        parent: &Self,
    ) -> Result<Self, CatalogError> {
        if selected.is_some() {
            return Ok(Self::new(selected, evidence));
        }
        evidence.extend(&parent.evidence)?;
        Ok(Self::new(parent.selected.clone(), evidence))
    }

    pub(crate) fn decision(&self) -> (Option<FactLayer>, &[SourceUse], bool) {
        let layer = self.selected.as_ref().map(|selected| selected.layer);
        let complete = layer.is_some() || !self.evidence.attempted;
        (layer, &self.evidence.selectors, complete)
    }

    pub(crate) fn bind(&self, sources: &SourceUses) -> Result<Stated<T>, CatalogError> {
        let (layer, selectors, complete) = self.decision();
        let layer = sources.value(
            layer.map_or_else(|| no_value(complete), SourceValue::Present),
            selectors,
            complete,
        );
        let value = if let Some(selected) = &self.selected {
            let mut inputs = Vec::new();
            extend(&mut inputs, selectors)?;
            inputs
                .try_reserve(1)
                .map_err(CatalogError::SourceUseAllocation)?;
            inputs.push(selected.datum.clone());
            sources.value(SourceValue::Present(selected.value), &inputs, complete)
        } else {
            sources.value(no_value(complete), selectors, complete)
        };
        Ok(Stated { value, layer })
    }
}

pub(crate) const fn no_value<T>(complete: bool) -> SourceValue<T> {
    if complete {
        SourceValue::Absent
    } else {
        SourceValue::Unreadable
    }
}
