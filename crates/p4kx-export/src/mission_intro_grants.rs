// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};

use p4kx_extraction::{
    Guid, PhysicalSourceRef, SharedFieldOrigin, SourceEvidenceError, SourceUseRef, SourceUseRole,
    SourceUses, SourceValue, SourcedValue, StructValue, Value,
};

use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::CatalogRecordContext;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueEdge};
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, ExportAccountingError};

use super::grant_read;
use super::read::{Evidence, Fact, Lookup, Reader};

pub(crate) const INTRO_CONTRACTS: &str = "introContracts";

type GrantList = SourcedValue<Vec<SourcedValue<CatalogGuid>>>;
type ById = BTreeMap<[u8; 36], Grant>;

struct Grant {
    id: CatalogGuid,
    evidence: Evidence,
}

/// Collects original grant decisions inside one Catalog input.
pub(crate) struct Grants {
    source: PhysicalSourceRef,
    uses: SourceUses,
    scope: Evidence,
    matches: HashMap<Guid, ById>,
    record: Option<Generator>,
}

struct Generator {
    index: u32,
    evidence: Evidence,
}

pub(crate) struct Candidates<'fields, 'scope, 'read, 'source> {
    collection: Option<Fact<'fields, 'scope, 'read, 'source>>,
    evidence: Evidence,
}

impl Grants {
    pub(crate) fn new(origins: &CatalogOrigins) -> Self {
        Self {
            source: origins.dataforge.source_ref().clone(),
            uses: origins.uses.clone(),
            scope: Evidence::default(),
            matches: HashMap::new(),
            record: None,
        }
    }

    fn check(&self, fields: &OriginalFlowFields<'_, '_, '_>) -> Result<(), CatalogError> {
        if !self.uses.is_same_inventory(fields.sources()) || &self.source != fields.input_ref() {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        fields.check_cancel()
    }

    pub(crate) fn unreadable_path(
        &mut self,
        context: &CatalogRecordContext<'_>,
        source_type: SourceUseRole,
    ) -> Result<(), CatalogError> {
        if !self.uses.is_same_inventory(context.sources())
            || &self.source != context.dataforge().source_ref()
            || context.record_position().is_none()
        {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        self.scope.retain(
            context.observe_header(RecordHeaderField::FileName, SourceUseRole::Attempted)?,
        )?;
        self.scope
            .retain(context.observe_header(RecordHeaderField::SourceType, source_type)?)
    }

    pub(crate) fn begin_record(
        &mut self,
        fields: &OriginalFlowFields<'_, '_, '_>,
        context: &CatalogRecordContext<'_>,
    ) -> Result<(), CatalogError> {
        self.check(fields)?;
        let index = context
            .record_position()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        fields.check_context(context.sources(), context.dataforge(), index)?;
        if self.record.is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let mut evidence = Evidence::default();
        for header in [RecordHeaderField::FileName, RecordHeaderField::SourceType] {
            evidence.retain(context.observe_header(header, SourceUseRole::Selector)?)?;
        }
        let _generators = Reader::new(fields).field(None, "generators", &mut evidence)?;
        self.scope.extend(&evidence)?;
        self.record = Some(Generator { index, evidence });
        Ok(())
    }

    pub(crate) fn handler_slot(
        &mut self,
        fields: &OriginalFlowFields<'_, '_, '_>,
        value: &Value<'_>,
    ) -> Result<(), CatalogError> {
        self.check(fields)?;
        self.scope.consult(fields.fact(value)?)
    }

    pub(crate) fn candidates<'fields, 'scope, 'read, 'source>(
        &mut self,
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        handler: &StructValue<'source>,
    ) -> Result<Candidates<'fields, 'scope, 'read, 'source>, CatalogError> {
        self.check(fields)?;
        let generator = self
            .record
            .as_ref()
            .filter(|record| record.index == fields.record_index())
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let reader = Reader::new(fields);
        let owner = reader.owner(handler)?;
        let mut evidence = Evidence::default();
        evidence.consult(owner)?;
        let collection = reader.field(Some(owner), INTRO_CONTRACTS, &mut evidence)?;
        self.scope.extend(&evidence)?;
        evidence.extend(&generator.evidence)?;
        Ok(Candidates {
            collection,
            evidence,
        })
    }

    pub(crate) fn candidate<'scope, 'read, 'source>(
        &mut self,
        fields: &OriginalFlowFields<'scope, 'read, 'source>,
        candidates: &Candidates<'_, 'scope, 'read, 'source>,
        value: &Value<'source>,
        admitted: Option<Guid>,
    ) -> Result<(), CatalogError> {
        self.check(fields)?;
        let reader = Reader::new(fields);
        let candidate = reader.fact(value)?;
        let parent = candidates
            .collection
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        fields.span(parent)?;
        let correct_parent = match parent.value() {
            Value::Struct(_) => std::ptr::eq(parent, candidate),
            Value::Array(_) => {
                candidate.parent_ordinal() == Some(parent.ordinal())
                    && matches!(candidate.edge(), OriginalValueEdge::Element { .. })
            }
            _ => false,
        };
        if !correct_parent {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let mut selected = Evidence::default();
        selected.consult(candidate)?;
        let id = if matches!(candidate.value(), Value::Struct(_)) {
            let _name = reader.field(Some(candidate), "debugName", &mut selected)?;
            reader.field(Some(candidate), "id", &mut selected)?
        } else {
            None
        };
        self.scope.extend(&selected)?;
        if let Some(admitted) = admitted {
            selected.extend(&candidates.evidence)?;
            grant_read::read(self, &reader, candidate, id, admitted, selected)?;
        }
        Ok(())
    }

    pub(super) fn survey(&mut self, evidence: &Evidence) -> Result<(), CatalogError> {
        self.scope.extend(evidence)
    }

    pub(super) fn add(
        &mut self,
        tag: Guid,
        id: CatalogGuid,
        evidence: Evidence,
    ) -> Result<(), CatalogError> {
        if !self.matches.contains_key(&tag) {
            self.matches
                .try_reserve(1)
                .map_err(CatalogError::SourceUseAllocation)?;
        }
        match self.matches.entry(tag).or_default().entry(id.text()) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(Grant { id, evidence });
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => {
                slot.get_mut().evidence.extend(&evidence)?;
            }
        }
        Ok(())
    }

    pub(crate) fn merge(&mut self, other: Self) -> Result<(), CatalogError> {
        if self.source != other.source || !self.uses.is_same_inventory(&other.uses) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        self.scope.extend(&other.scope)?;
        for (tag, ids) in other.matches {
            for grant in ids.into_values() {
                self.add(tag, grant.id, grant.evidence)?;
            }
        }
        Ok(())
    }

    pub(crate) const fn incomplete(&mut self) {
        self.scope.incomplete();
    }

    pub(super) fn resolver(self) -> Resolver {
        let Self {
            source,
            uses,
            scope,
            matches,
            ..
        } = self;
        let scope = scope.shared_origin(&uses);
        Resolver {
            source,
            uses,
            scope,
            matches,
            resolved: HashMap::new(),
        }
    }
}

pub(super) struct Resolver {
    source: PhysicalSourceRef,
    uses: SourceUses,
    scope: SharedFieldOrigin,
    matches: HashMap<Guid, ById>,
    resolved: HashMap<SourceUseRef, GrantList>,
}

impl Resolver {
    pub(super) fn grants(&mut self, lookup: &Lookup) -> Result<GrantList, CatalogError> {
        if !self.uses.is_same_inventory(&lookup.sources)
            || lookup.caller.description().source() != &self.source
        {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        let key = lookup.caller.reference();
        if let Some(resolved) = self.resolved.get(key) {
            return Ok(resolved.clone());
        }
        let mut values = Vec::new();
        if let Some(ids) = self.matches.get(&lookup.guid) {
            values
                .try_reserve_exact(ids.len())
                .map_err(CatalogError::SourceUseAllocation)?;
            for grant in ids.values() {
                values.push(grant.evidence.bind_lookup(&self.uses, grant.id, lookup)?);
            }
        }
        let (selectors, complete) = lookup.contributions();
        let resolved = self.uses.value_with_shared_origin(
            SourceValue::Present(values),
            &self.scope,
            selectors,
            complete,
        )?;
        self.resolved
            .try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        self.resolved.insert(key.clone(), resolved.clone());
        Ok(resolved)
    }
}
