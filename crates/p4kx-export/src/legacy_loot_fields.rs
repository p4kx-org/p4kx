// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue, Value};

use crate::CatalogError;
use crate::catalog_read::original::{
    OriginalFlowFields, OriginalValueEdge, OriginalValueFact, OriginalValuePart,
};
use crate::guid_text::CatalogGuid;
use crate::record_fields::{integer_value, number_value, record_reference};
use crate::resource_number::ResourceNumber;

pub(super) type Fact<'scope, 'read, 'source> = OriginalValueFact<'scope, 'read, 'source>;
pub(super) type Slots<T> = SourcedValue<Vec<SourcedValue<T>>>;

pub(super) struct Fields<'facts, 'scope, 'read, 'source> {
    pub(super) facts: &'facts OriginalFlowFields<'scope, 'read, 'source>,
}

impl<'scope, 'read, 'source> Fields<'_, 'scope, 'read, 'source> {
    pub(super) fn field<T>(
        &self,
        owner: Option<&Fact<'scope, 'read, 'source>>,
        name: &str,
        value: impl FnOnce(&Fact<'scope, 'read, 'source>) -> Result<Option<T>, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some((_, fact)) = self.facts.field(owner, name)? else {
            return self.absent(owner);
        };
        self.cell(fact, value)
    }

    pub(super) fn absent<T>(
        &self,
        owner: Option<&Fact<'scope, 'read, 'source>>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let survey = self.facts.survey(owner)?;
        Ok(self
            .facts
            .sources()
            .value(SourceValue::Absent, &[survey], true))
    }

    pub(super) fn cell<T>(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        value: impl FnOnce(&Fact<'scope, 'read, 'source>) -> Result<Option<T>, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let state = match fact.value() {
            Value::Null => SourceValue::Null,
            Value::Error(_) => SourceValue::Unreadable,
            _ => value(fact)?.map_or(SourceValue::Invalid, SourceValue::Present),
        };
        self.bind(fact, state)
    }

    pub(super) fn bind<T>(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        state: SourceValue<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.bind_selected(fact, state, Vec::new())
    }

    pub(super) fn bind_selected<T>(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        state: SourceValue<T>,
        mut uses: Vec<SourceUse>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let role = match state {
            SourceValue::Present(_) => SourceUseRole::Payload,
            SourceValue::Unreadable => SourceUseRole::Attempted,
            _ => SourceUseRole::Selector,
        };
        uses.push(fact.observe(OriginalValuePart::Value, role)?);
        Self::select_field_name(fact, &mut uses)?;
        let complete = uses
            .iter()
            .all(|used| used.role() != SourceUseRole::Attempted);
        Ok(self.facts.sources().value(state, &uses, complete))
    }

    fn select_field_name(
        fact: &Fact<'scope, 'read, 'source>,
        uses: &mut Vec<SourceUse>,
    ) -> Result<(), CatalogError> {
        if matches!(fact.edge(), OriginalValueEdge::Field { .. }) {
            uses.push(fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
        }
        Ok(())
    }

    pub(super) fn selection_use(
        fact: &Fact<'scope, 'read, 'source>,
    ) -> Result<SourceUse, CatalogError> {
        let (part, role) = match fact.value() {
            Value::Struct(_) => (OriginalValuePart::ConcreteType, SourceUseRole::Selector),
            Value::Error(_) => (OriginalValuePart::Value, SourceUseRole::Attempted),
            _ => (OriginalValuePart::Value, SourceUseRole::Selector),
        };
        fact.observe(part, role)
    }

    pub(super) fn inspect_selector(
        &self,
        owner: &Fact<'scope, 'read, 'source>,
        name: &str,
        uses: &mut Vec<SourceUse>,
    ) -> Result<(), CatalogError> {
        if let Some((_, fact)) = self.facts.field(Some(owner), name)? {
            uses.push(fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
            uses.push(Self::selection_use(fact)?);
        } else {
            uses.push(self.facts.survey(Some(owner))?);
        }
        Ok(())
    }
    pub(super) fn slots<T>(
        &self,
        owner: Option<&Fact<'scope, 'read, 'source>>,
        name: &str,
        item: impl Fn(&Fact<'scope, 'read, 'source>) -> Result<SourcedValue<T>, CatalogError>,
    ) -> Result<Slots<T>, CatalogError> {
        self.field(owner, name, |array| {
            if !matches!(array.value(), Value::Array(_)) {
                return Ok(None);
            }
            let mut rows = Vec::new();
            for slot in self.facts.elements(array)? {
                rows.push(item(slot)?);
            }
            Ok(Some(rows))
        })
    }

    pub(super) fn number(
        &self,
        owner: &Fact<'scope, 'read, 'source>,
        name: &str,
    ) -> Result<SourcedValue<ResourceNumber>, CatalogError> {
        self.field(Some(owner), name, |fact| {
            Ok(number_value(fact.value()).and_then(|_| ResourceNumber::read(fact.value())))
        })
    }

    pub(super) fn integer(
        &self,
        owner: &Fact<'scope, 'read, 'source>,
        name: &str,
    ) -> Result<SourcedValue<ResourceNumber>, CatalogError> {
        self.field(Some(owner), name, |fact| {
            Ok(integer_value(fact.value()).and_then(|_| ResourceNumber::read(fact.value())))
        })
    }

    pub(super) fn reference(
        &self,
        owner: &Fact<'scope, 'read, 'source>,
        name: &str,
    ) -> Result<SourcedValue<CatalogGuid>, CatalogError> {
        let Some((_, fact)) = self.facts.field(Some(owner), name)? else {
            return self.absent(Some(owner));
        };
        self.reference_cell(fact)
    }

    fn reference_cell(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
    ) -> Result<SourcedValue<CatalogGuid>, CatalogError> {
        if let Some(authored) = fact.dangling_guid(SourceUseRole::Payload)? {
            let (value, mut uses, _) = authored.map(CatalogGuid)?.into_parts();
            uses.push(fact.observe(OriginalValuePart::Value, SourceUseRole::Attempted)?);
            Self::select_field_name(fact, &mut uses)?;
            return Ok(self.facts.sources().value(value, &uses, false));
        }
        self.cell(fact, |fact| Ok(record_reference(fact.value())))
    }

    pub(super) fn references(
        &self,
        owner: &Fact<'scope, 'read, 'source>,
        name: &str,
    ) -> Result<Slots<CatalogGuid>, CatalogError> {
        self.slots(Some(owner), name, |fact| self.reference_cell(fact))
    }

    pub(super) fn label(
        &self,
        owner: &Fact<'scope, 'read, 'source>,
        name: &str,
    ) -> Result<SourcedValue<&'source str>, CatalogError> {
        self.field(Some(owner), name, |fact| {
            Ok(match fact.value() {
                Value::String(text) | Value::Enum(text) => Some(*text),
                _ => None,
            })
        })
    }
}
