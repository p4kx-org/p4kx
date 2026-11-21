// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, Value};

use crate::catalog_read::original::{
    OriginalFlowFields, OriginalValueEdge, OriginalValueFact, OriginalValuePart,
};
use crate::{CatalogError, ExportAccountingError};

impl<'scope, 'read, 'source> OriginalFlowFields<'scope, 'read, 'source> {
    pub(crate) fn elements(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<Vec<&OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
        self.span(fact)?;
        let Value::Array(array) = fact.value() else {
            return Ok(Vec::new());
        };
        array
            .iter()
            .enumerate()
            .map(|(index, _)| self.element(fact, index))
            .collect()
    }

    pub(crate) fn structs(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<Vec<&OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
        self.span(fact)?;
        let Value::Array(array) = fact.value() else {
            return Ok(Vec::new());
        };
        let mut found = Vec::new();
        for (index, value) in array.iter().enumerate() {
            self.check_cancel()?;
            if matches!(value, Value::Struct(_)) {
                found.push(self.element(fact, index)?);
            }
        }
        Ok(found)
    }

    pub(crate) fn first_struct(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<Option<&OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
        self.first_struct_observed(fact, |_| Ok(()))
    }

    /// Returns the first struct and observes the array prefix used to select it.
    pub(crate) fn first_struct_observed(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        mut observe: impl FnMut(&Value<'source>) -> Result<(), CatalogError>,
    ) -> Result<Option<&OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
        self.span(fact)?;
        match fact.value() {
            Value::Struct(_) => Ok(Some(self.fact(fact.value())?)),
            Value::Array(array) => {
                for (index, value) in array.iter().enumerate() {
                    self.check_cancel()?;
                    observe(value)?;
                    if matches!(value, Value::Struct(_)) {
                        return Ok(Some(self.element(fact, index)?));
                    }
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    pub(crate) fn selected_struct(
        &self,
        parent: &OriginalValueFact<'scope, 'read, 'source>,
        name: &str,
    ) -> Result<Option<&OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
        self.field(Some(parent), name)?
            .map(|(_, fact)| self.first_struct(fact))
            .transpose()
            .map(Option::flatten)
    }

    /// Reads the first mission outcome and retains the original inputs that decide success.
    pub(crate) fn first_mission_outcome(
        &self,
        result: &OriginalValueFact<'scope, 'read, 'source>,
        mut retain: impl FnMut(SourceUse) -> Result<(), CatalogError>,
    ) -> Result<bool, CatalogError> {
        let Some(outcomes) = self.observe_field_selection(result, "missionResults", &mut retain)?
        else {
            return Ok(false);
        };
        let first = match outcomes.value() {
            Value::Array(values) => {
                let Some(first) = values.first() else {
                    return Ok(false);
                };
                let fact = self.fact(first)?;
                retain(selection_use(fact)?)?;
                fact
            }
            Value::Struct(_) => outcomes,
            _ => return Ok(false),
        };
        match first.value() {
            Value::Bool(value) => Ok(*value),
            Value::Struct(_) => Ok(self
                .observe_field_selection(first, "value", &mut retain)?
                .is_some_and(|fact| matches!(fact.value(), Value::Bool(true)))),
            _ => Ok(false),
        }
    }

    /// Observes the declaration or its owning declaration survey.
    pub(crate) fn observe_field_declaration(
        &self,
        owner: &OriginalValueFact<'scope, 'read, 'source>,
        name: &str,
        retain: &mut impl FnMut(SourceUse) -> Result<(), CatalogError>,
    ) -> Result<Option<&OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
        let Some((_, fact)) = self.field(Some(owner), name)? else {
            retain(self.survey(Some(owner))?)?;
            return Ok(None);
        };
        retain(fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?)?;
        Ok(Some(fact))
    }

    /// Observes a field declaration and the value used in a selection.
    pub(crate) fn observe_field_selection(
        &self,
        owner: &OriginalValueFact<'scope, 'read, 'source>,
        name: &str,
        retain: &mut impl FnMut(SourceUse) -> Result<(), CatalogError>,
    ) -> Result<Option<&OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
        let fact = self.observe_field_declaration(owner, name, retain)?;
        if let Some(fact) = fact {
            retain(selection_use(fact)?)?;
        }
        Ok(fact)
    }

    /// Observes the field and prefix used to select its first struct.
    pub(crate) fn observe_struct_selection(
        &self,
        owner: &OriginalValueFact<'scope, 'read, 'source>,
        name: &str,
        mut retain: impl FnMut(SourceUse) -> Result<(), CatalogError>,
    ) -> Result<Option<&OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
        let Some(fact) = self.observe_field_selection(owner, name, &mut retain)? else {
            return Ok(None);
        };
        self.first_struct_observed(fact, |value| retain(selection_use(self.fact(value)?)?))
    }

    fn element(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        index: usize,
    ) -> Result<&OriginalValueFact<'scope, 'read, 'source>, CatalogError> {
        self.child(
            Some(fact.ordinal()),
            OriginalValueEdge::Element {
                ordinal: u64::try_from(index)
                    .map_err(|_| ExportAccountingError::CounterOverflow)?,
            },
        )
    }
}

fn selection_use(fact: &OriginalValueFact<'_, '_, '_>) -> Result<SourceUse, CatalogError> {
    let role = if matches!(fact.value(), Value::Error(_)) {
        SourceUseRole::Attempted
    } else {
        SourceUseRole::Selector
    };
    fact.observe(OriginalValuePart::Value, role)
}
