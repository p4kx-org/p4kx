// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use p4kx_extraction::{
    SourceEvidenceError, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
    StructValue, Value,
};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::mission_layers;

pub(super) type Fact<'fields, 'scope, 'read, 'source> =
    &'fields OriginalValueFact<'scope, 'read, 'source>;

#[derive(Debug)]
struct TextOrigin {
    inventory: SourceUses,
    dataforge: SourceScope,
}

/// One selected original key and its deferred use sites.
#[derive(Clone, Debug)]
pub(crate) struct MissionKey<'source> {
    pub(crate) text: &'source str,
    origin: Arc<TextOrigin>,
    location: SourceLocation,
}

pub(crate) struct SelectedKey {
    pub(crate) payload: SourceUse,
    pub(crate) selector: SourceUse,
}

impl MissionKey<'_> {
    pub(crate) fn retain(&self, origins: &CatalogOrigins) -> Result<SelectedKey, CatalogError> {
        let selector = self.selector(origins)?;
        Ok(SelectedKey {
            payload: self
                .origin
                .dataforge
                .at(self.location.clone(), SourceUseRole::Payload)?,
            selector,
        })
    }

    pub(crate) fn selector(&self, origins: &CatalogOrigins) -> Result<SourceUse, CatalogError> {
        self.origin.check(origins)?;
        Ok(self
            .origin
            .dataforge
            .at(self.location.clone(), SourceUseRole::Selector)?)
    }
}

#[derive(Debug)]
struct Step {
    location: SourceLocation,
    role: SourceUseRole,
}

/// Original decision fields retained only when the actual branch is used.
#[derive(Debug)]
pub(crate) struct Decisions {
    origin: Arc<TextOrigin>,
    steps: Vec<Step>,
}

impl Decisions {
    pub(crate) fn retain(
        &self,
        origins: &CatalogOrigins,
        into: &mut Vec<SourceUse>,
    ) -> Result<(), CatalogError> {
        self.retain_range(0, self.steps.len(), origins, into)
    }

    fn retain_range(
        &self,
        start: usize,
        end: usize,
        origins: &CatalogOrigins,
        into: &mut Vec<SourceUse>,
    ) -> Result<(), CatalogError> {
        self.origin.check(origins)?;
        let steps = self
            .steps
            .get(start..end)
            .ok_or(CatalogError::MissionTextPlan)?;
        into.try_reserve(steps.len())
            .map_err(CatalogError::SourceUseAllocation)?;
        for step in steps {
            into.push(self.origin.dataforge.at(step.location.clone(), step.role)?);
        }
        Ok(())
    }

    pub(crate) fn unreadable(&self) -> bool {
        self.steps
            .iter()
            .any(|step| step.role == SourceUseRole::Attempted)
    }

    pub(super) const fn len(&self) -> usize {
        self.steps.len()
    }

    fn push(&mut self, location: SourceLocation, role: SourceUseRole) -> Result<(), CatalogError> {
        self.steps
            .try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        self.steps.push(Step { location, role });
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct DecisionIdentity<'read>(&'read Decisions);

impl PartialEq for DecisionIdentity<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl Eq for DecisionIdentity<'_> {}

impl Hash for DecisionIdentity<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.0, state);
    }
}

/// The original inputs consulted for one language and rendered field.
pub(crate) struct Consulted<'read> {
    uses: Vec<SourceUse>,
    prefixes: HashMap<DecisionIdentity<'read>, usize>,
}

impl<'read> Consulted<'read> {
    pub(crate) fn new(initial: &[SourceUse]) -> Result<Self, CatalogError> {
        let mut uses = Vec::new();
        uses.try_reserve(initial.len())
            .map_err(CatalogError::SourceUseAllocation)?;
        uses.extend_from_slice(initial);
        Ok(Self {
            uses,
            prefixes: HashMap::new(),
        })
    }

    pub(crate) fn all(
        &mut self,
        decisions: &'read Decisions,
        origins: &CatalogOrigins,
    ) -> Result<(), CatalogError> {
        self.prefix(decisions, decisions.len(), origins)
    }

    pub(crate) fn prefix(
        &mut self,
        decisions: &'read Decisions,
        end: usize,
        origins: &CatalogOrigins,
    ) -> Result<(), CatalogError> {
        let id = DecisionIdentity(decisions);
        let start = self.prefixes.get(&id).copied().unwrap_or_default();
        if end <= start {
            return Ok(());
        }
        if start == 0 {
            self.prefixes
                .try_reserve(1)
                .map_err(CatalogError::SourceUseAllocation)?;
        }
        decisions.retain_range(start, end, origins, &mut self.uses)?;
        self.prefixes.insert(id, end);
        Ok(())
    }

    pub(crate) const fn uses(&self) -> &[SourceUse] {
        self.uses.as_slice()
    }

    pub(crate) const fn uses_mut(&mut self) -> &mut Vec<SourceUse> {
        &mut self.uses
    }
}

impl TextOrigin {
    fn check(&self, origins: &CatalogOrigins) -> Result<(), CatalogError> {
        if !self.inventory.is_same_inventory(&origins.uses)
            || self.dataforge.source_ref() != origins.dataforge.source_ref()
        {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(())
    }
}

/// The actual original scope used by one mission text dependency or generator layer read.
pub(super) struct Fields<'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    origin: Arc<TextOrigin>,
}

impl<'fields, 'scope, 'read, 'source> Fields<'fields, 'scope, 'read, 'source> {
    pub(super) fn new(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        origins: &CatalogOrigins,
    ) -> Result<Self, CatalogError> {
        fields.check_context(&origins.uses, &origins.dataforge, fields.record_index())?;
        Ok(Self {
            fields,
            origin: Arc::new(TextOrigin {
                inventory: origins.uses.clone(),
                dataforge: origins.dataforge.clone(),
            }),
        })
    }

    pub(super) fn decisions(&self) -> Decisions {
        Decisions {
            origin: Arc::clone(&self.origin),
            steps: Vec::new(),
        }
    }

    pub(super) fn owner(
        &self,
        value: &StructValue<'source>,
    ) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
        self.fields.owner_of(value)
    }

    pub(super) fn field(
        &self,
        parent: Option<Fact<'fields, 'scope, 'read, 'source>>,
        name: &str,
        decisions: &mut Decisions,
    ) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
        let value = self.fields.field(parent, name)?.map(|(_, value)| value);
        if let Some(value) = value {
            self.record(value, OriginalValuePart::Value, decisions)?;
        } else {
            self.check(decisions)?;
            decisions.push(
                self.fields.survey_location(parent)?,
                SourceUseRole::Selector,
            )?;
        }
        Ok(value)
    }

    pub(super) fn selected(
        &self,
        value: Fact<'fields, 'scope, 'read, 'source>,
        decisions: &mut Decisions,
    ) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
        let selected = self.fields.first_struct(value)?;
        if let Some(selected) = selected {
            self.record(selected, OriginalValuePart::ConcreteType, decisions)?;
        }
        Ok(selected)
    }

    pub(super) fn items(
        &self,
        value: Fact<'fields, 'scope, 'read, 'source>,
    ) -> Result<Vec<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
        if matches!(value.value(), Value::Struct(_)) {
            self.fields.span(value)?;
            Ok(vec![value])
        } else {
            self.fields.elements(value)
        }
    }

    pub(super) fn record(
        &self,
        value: Fact<'fields, 'scope, 'read, 'source>,
        part: OriginalValuePart,
        decisions: &mut Decisions,
    ) -> Result<(), CatalogError> {
        self.check(decisions)?;
        self.fields.span(value)?;
        let role = if matches!(value.value(), Value::Error(_)) {
            SourceUseRole::Attempted
        } else {
            SourceUseRole::Selector
        };
        decisions.push(value.location(part), role)
    }

    pub(super) fn key(
        &self,
        fact: Fact<'fields, 'scope, 'read, 'source>,
    ) -> Result<Option<MissionKey<'source>>, CatalogError> {
        self.fields.span(fact)?;
        Ok(
            mission_layers::key_value(fact.value()).map(|text| MissionKey {
                text,
                origin: Arc::clone(&self.origin),
                location: fact.location(OriginalValuePart::Value),
            }),
        )
    }

    fn check(&self, decisions: &Decisions) -> Result<(), CatalogError> {
        if !Arc::ptr_eq(&self.origin, &decisions.origin) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "mission_text_evidence_tests.rs"]
mod tests;
