// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Guid, SharedFieldOrigin, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
    StructValue, Value, ValueError,
};

use crate::catalog_read::original::{
    OriginalDatum, OriginalErrorParameter, OriginalFlowFields, OriginalReferenceObservation,
    OriginalValueFact, OriginalValuePart,
};
use crate::record_fields;
use crate::{CatalogError, ExportAccountingError};

pub(super) type Fact<'fields, 'scope, 'read, 'source> =
    &'fields OriginalValueFact<'scope, 'read, 'source>;

#[derive(Debug, Clone, Default)]
pub(super) struct Evidence {
    uses: Vec<SourceUse>,
    incomplete: bool,
}

impl Evidence {
    pub(super) fn retain(&mut self, used: SourceUse) -> Result<(), CatalogError> {
        if used.role() == SourceUseRole::Attempted {
            self.incomplete = true;
        } else {
            self.uses
                .try_reserve(1)
                .map_err(CatalogError::SourceUseAllocation)?;
            self.uses.push(used);
        }
        Ok(())
    }

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

    pub(super) fn extend(&mut self, other: &Self) -> Result<(), CatalogError> {
        self.extend_uses(&other.uses)?;
        self.incomplete |= other.incomplete;
        Ok(())
    }

    pub(super) fn extend_uses(&mut self, uses: &[SourceUse]) -> Result<(), CatalogError> {
        self.uses
            .try_reserve(uses.len())
            .map_err(CatalogError::SourceUseAllocation)?;
        self.uses.extend_from_slice(uses);
        Ok(())
    }

    pub(super) const fn incomplete(&mut self) {
        self.incomplete = true;
    }

    pub(super) fn bind<T>(&self, sources: &SourceUses, value: T) -> SourcedValue<T> {
        sources.value(SourceValue::Present(value), &self.uses, !self.incomplete)
    }

    pub(super) fn shared_origin(&self, sources: &SourceUses) -> SharedFieldOrigin {
        sources.shared_origin(&self.uses, !self.incomplete)
    }

    pub(super) fn bind_lookup<T>(
        &self,
        sources: &SourceUses,
        value: T,
        lookup: &Lookup,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let (selectors, complete) = lookup.contributions();
        let size = self
            .uses
            .len()
            .checked_add(selectors.len())
            .ok_or(ExportAccountingError::CounterOverflow)?;
        let mut uses = Vec::new();
        uses.try_reserve_exact(size)
            .map_err(CatalogError::SourceUseAllocation)?;
        uses.extend_from_slice(&self.uses);
        uses.extend_from_slice(selectors);
        Ok(sources.value(
            SourceValue::Present(value),
            &uses,
            !self.incomplete && complete,
        ))
    }
}

pub(super) struct Reader<'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
}

impl<'fields, 'scope, 'read, 'source> Reader<'fields, 'scope, 'read, 'source> {
    pub(super) const fn new(fields: &'fields OriginalFlowFields<'scope, 'read, 'source>) -> Self {
        Self { fields }
    }

    pub(super) const fn sources(&self) -> &SourceUses {
        self.fields.sources()
    }

    pub(super) fn owner(
        &self,
        source: &StructValue<'source>,
    ) -> Result<Fact<'fields, 'scope, 'read, 'source>, CatalogError> {
        self.fields
            .owner_of(source)?
            .ok_or_else(|| ExportAccountingError::ConflictingSourceDatum.into())
    }

    pub(super) fn fact(
        &self,
        value: &Value<'source>,
    ) -> Result<Fact<'fields, 'scope, 'read, 'source>, CatalogError> {
        self.fields.fact(value)
    }

    pub(super) fn field(
        &self,
        parent: Option<Fact<'fields, 'scope, 'read, 'source>>,
        name: &str,
        evidence: &mut Evidence,
    ) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
        let fact = self.declaration(parent, name, evidence)?;
        if let Some(fact) = fact {
            evidence.consult(fact)?;
        }
        Ok(fact)
    }

    pub(super) fn declaration(
        &self,
        parent: Option<Fact<'fields, 'scope, 'read, 'source>>,
        name: &str,
        evidence: &mut Evidence,
    ) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
        let Some((_, fact)) = self.fields.field(parent, name)? else {
            evidence.retain(self.fields.survey(parent)?)?;
            return Ok(None);
        };
        evidence.retain(fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?)?;
        Ok(Some(fact))
    }

    pub(super) fn first_mission_outcome(
        &self,
        result: Fact<'fields, 'scope, 'read, 'source>,
        evidence: &mut Evidence,
    ) -> Result<bool, CatalogError> {
        self.fields
            .first_mission_outcome(result, |used| evidence.retain(used))
    }

    pub(super) fn first_struct(
        &self,
        field: Fact<'fields, 'scope, 'read, 'source>,
        evidence: &mut Evidence,
    ) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
        self.fields
            .first_struct_observed(field, |value| evidence.consult(self.fact(value)?))
    }
}

pub(super) struct Tag<'fact, 'scope, 'read, 'source> {
    pub(super) observed: OriginalReferenceObservation<'fact, 'scope, 'read, 'source>,
    pub(super) guid: Guid,
    selected: OriginalDatum<Guid>,
}

impl Tag<'_, '_, '_, '_> {
    pub(super) fn lookup(self, sources: &SourceUses) -> Result<Lookup, CatalogError> {
        let caller = self
            .observed
            .caller()
            .observe(OriginalValuePart::Value, SourceUseRole::Selector)?;
        Ok(Lookup {
            caller,
            sources: sources.clone(),
            selected: self.selected,
            guid: self.guid,
        })
    }
}

/// The exact authored Reference that selects an intro grant lookup.
#[derive(Debug, Clone)]
pub(super) struct Lookup {
    pub(super) caller: SourceUse,
    pub(super) sources: SourceUses,
    pub(super) guid: Guid,
    selected: OriginalDatum<Guid>,
}

impl Lookup {
    pub(super) fn contributions(&self) -> (&[SourceUse], bool) {
        self.selected.contributions()
    }
}

pub(super) fn tag<'fact, 'scope, 'read, 'source>(
    fact: &'fact OriginalValueFact<'scope, 'read, 'source>,
    evidence: &mut Evidence,
) -> Result<Option<Tag<'fact, 'scope, 'read, 'source>>, CatalogError> {
    if matches!(
        fact.value(),
        Value::Error(ValueError::DanglingReference { .. })
    ) {
        evidence.retain(fact.observe(
            OriginalValuePart::Error(OriginalErrorParameter::Guid),
            SourceUseRole::Selector,
        )?)?;
        return Ok(None);
    }
    let Some(guid) = record_fields::record_reference(fact.value()) else {
        evidence.consult(fact)?;
        return Ok(None);
    };
    let observed = fact.reference_observation()?;
    let selected = observed.guid_selector()?;
    if selected.value() != &SourceValue::Present(guid.0) {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let (uses, complete) = selected.contributions();
    evidence.extend_uses(uses)?;
    if !complete {
        evidence.incomplete();
    }
    Ok(Some(Tag {
        observed,
        guid: guid.0,
        selected,
    }))
}
