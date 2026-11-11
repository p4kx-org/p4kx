// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue, StructValue, Value};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{
    OriginalDatum, OriginalFlowFields, OriginalReferenceObservation, OriginalValueFact,
    OriginalValuePart,
};
use crate::contract_missions::ReputationGain;
use crate::guid_text::CatalogGuid;
use crate::mission_facts::choice;
use crate::mission_index::MissionIndex;
use crate::mission_layers::items_value;
use crate::resource_integer::ResourceInt64;
use crate::{CatalogError, ExportAccountingError};

const RESULT_BLOCK: &str = "contractResults";
const LEGACY_REPUTATION: &str = "ContractResult_LegacyReputation";

/// Reads the successful reputation results in the selected contract or sub-contract block.
pub(crate) fn gains<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    index: &MissionIndex<'source>,
    parent: &StructValue<'source>,
    sub: Option<&StructValue<'source>>,
) -> Result<SourcedValue<Vec<ReputationGain<'source>>>, CatalogError> {
    let reader = Reader { fields };
    let mut path = Vec::new();
    let results = reader.selected_results(parent, sub, &mut path)?;
    let mut membership = Vec::new();
    choice::extend(&mut membership, &path)?;
    let mut gains = Vec::new();
    if let Some(results) = results {
        for value in items_value(results.value()) {
            let candidate = fields.fact(value)?;
            let mut branch = Vec::new();
            let amounts = reader.selected_amounts(candidate, &mut branch)?;
            choice::extend(&mut membership, &branch)?;
            if let Some(amounts) = amounts {
                choice::extend(&mut branch, &path)?;
                reader.append_amounts(
                    amounts,
                    &branch,
                    (input, index),
                    (&mut gains, &mut membership),
                )?;
            }
        }
    }
    Ok(reader.bind(SourceValue::Present(gains), &membership))
}

type Fact<'fields, 'scope, 'read, 'source> = &'fields OriginalValueFact<'scope, 'read, 'source>;

struct Reader<'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
}

impl<'fields, 'scope, 'read, 'source> Reader<'fields, 'scope, 'read, 'source> {
    fn selected_results(
        &self,
        parent: &StructValue<'source>,
        sub: Option<&StructValue<'source>>,
        uses: &mut Vec<SourceUse>,
    ) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
        if let Some(sub) = sub
            && let Some(results) = self.result_list(sub, uses)?
            && !items_value(results.value()).is_empty()
        {
            return Ok(Some(results));
        }
        self.result_list(parent, uses)
    }

    fn result_list(
        &self,
        owner: &StructValue<'source>,
        uses: &mut Vec<SourceUse>,
    ) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
        let owner = self
            .fields
            .owner_of(owner)?
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let Some(block) = self
            .fields
            .observe_struct_selection(owner, RESULT_BLOCK, |used| retain(uses, used))?
        else {
            return Ok(None);
        };
        self.fields
            .observe_field_selection(block, RESULT_BLOCK, &mut |used| retain(uses, used))
    }

    fn selected_amounts(
        &self,
        candidate: Fact<'fields, 'scope, 'read, 'source>,
        uses: &mut Vec<SourceUse>,
    ) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
        consult(uses, candidate)?;
        let Value::Struct(result) = candidate.value() else {
            return Ok(None);
        };
        retain(
            uses,
            candidate.observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector)?,
        )?;
        if result.type_name() != LEGACY_REPUTATION
            || !self
                .fields
                .first_mission_outcome(candidate, |used| retain(uses, used))?
        {
            return Ok(None);
        }
        self.fields.observe_field_selection(
            candidate,
            "contractResultReputationAmounts",
            &mut |used| retain(uses, used),
        )
    }

    fn append_amounts(
        &self,
        amounts: Fact<'fields, 'scope, 'read, 'source>,
        path: &[SourceUse],
        dependencies: (&OriginalCatalogInput<'_, 'source>, &MissionIndex<'source>),
        (gains, membership): (&mut Vec<ReputationGain<'source>>, &mut Vec<SourceUse>),
    ) -> Result<(), CatalogError> {
        for value in items_value(amounts.value()) {
            let entry = self.fields.fact(value)?;
            let mut branch = Vec::new();
            consult(&mut branch, entry)?;
            choice::extend(membership, &branch)?;
            if matches!(entry.value(), Value::Struct(_)) {
                choice::extend(&mut branch, path)?;
                gains
                    .try_reserve(1)
                    .map_err(CatalogError::SourceUseAllocation)?;
                gains.push(self.gain(entry, &branch, dependencies)?);
            }
        }
        Ok(())
    }

    fn gain(
        &self,
        entry: Fact<'fields, 'scope, 'read, 'source>,
        path: &[SourceUse],
        (input, index): (&OriginalCatalogInput<'_, 'source>, &MissionIndex<'source>),
    ) -> Result<ReputationGain<'source>, CatalogError> {
        let (faction_reputation_guid, faction_reputation_name) =
            self.link(entry, "factionReputation", path)?;
        let (scope_guid, scope_name) = self.link(entry, "reputationScope", path)?;
        let mut uses = Vec::new();
        choice::extend(&mut uses, path)?;
        let amount = match self.reference(entry, "reward", &mut uses)? {
            ReferenceRead::Present(reference) => {
                index.nominal_reward_amount(input, &reference, &mut uses)?
            }
            ReferenceRead::Missing(state) => state.value(),
        };
        Ok(ReputationGain {
            faction_reputation_guid,
            faction_reputation_name,
            scope_guid,
            scope_name,
            amount: self.bind(amount, &uses).map(ResourceInt64::new),
        })
    }

    fn link(
        &self,
        entry: Fact<'fields, 'scope, 'read, 'source>,
        name: &str,
        path: &[SourceUse],
    ) -> Result<Link<'source>, CatalogError> {
        let mut uses = Vec::new();
        choice::extend(&mut uses, path)?;
        match self.reference(entry, name, &mut uses)? {
            ReferenceRead::Present(reference) => {
                let guid = reference.guid()?;
                let name = reference.record_path()?;
                Ok((
                    self.bind_datum(&guid, &uses)?.map(CatalogGuid),
                    self.bind_datum(&name, &uses)?
                        .map(|path| path.rsplit('/').next().unwrap_or(path)),
                ))
            }
            ReferenceRead::Missing(state) => Ok((
                self.bind(state.value(), &uses),
                self.bind(state.value(), &uses),
            )),
        }
    }

    fn reference(
        &self,
        entry: Fact<'fields, 'scope, 'read, 'source>,
        name: &str,
        uses: &mut Vec<SourceUse>,
    ) -> Result<
        ReferenceRead<OriginalReferenceObservation<'fields, 'scope, 'read, 'source>>,
        CatalogError,
    > {
        let Some(fact) = self
            .fields
            .observe_field_declaration(entry, name, &mut |used| retain(uses, used))?
        else {
            return Ok(ReferenceRead::Missing(MissingReference::Absent));
        };
        if matches!(fact.value(), Value::Reference(reference) if reference.location().is_none()) {
            return Ok(ReferenceRead::Present(fact.reference_observation()?));
        }
        if let Some(guid) = fact.dangling_guid(SourceUseRole::Selector)? {
            choice::extend(uses, guid.contributions().0)?;
            return Ok(ReferenceRead::Missing(MissingReference::Invalid));
        }
        consult(uses, fact)?;
        let state = match fact.value() {
            Value::Null => MissingReference::Null,
            Value::Error(_) => MissingReference::Unreadable,
            _ => MissingReference::Invalid,
        };
        Ok(ReferenceRead::Missing(state))
    }

    fn bind_datum<T: Copy>(
        &self,
        datum: &OriginalDatum<T>,
        path: &[SourceUse],
    ) -> Result<SourcedValue<T>, CatalogError> {
        let mut uses = Vec::new();
        choice::extend(&mut uses, path)?;
        let (datum_uses, complete) = datum.contributions();
        choice::extend(&mut uses, datum_uses)?;
        Ok(self.fields.sources().value(
            datum.value().clone(),
            &uses,
            complete && selection_complete(path),
        ))
    }

    fn bind<T>(&self, value: SourceValue<T>, uses: &[SourceUse]) -> SourcedValue<T> {
        self.fields
            .sources()
            .value(value, uses, selection_complete(uses))
    }
}

type Link<'source> = (SourcedValue<CatalogGuid>, SourcedValue<&'source str>);

enum ReferenceRead<T> {
    Present(T),
    Missing(MissingReference),
}

#[derive(Clone, Copy)]
enum MissingReference {
    Absent,
    Null,
    Unreadable,
    Invalid,
}

impl MissingReference {
    const fn value<T>(self) -> SourceValue<T> {
        match self {
            Self::Absent => SourceValue::Absent,
            Self::Null => SourceValue::Null,
            Self::Unreadable => SourceValue::Unreadable,
            Self::Invalid => SourceValue::Invalid,
        }
    }
}

fn selection_complete(uses: &[SourceUse]) -> bool {
    !uses
        .iter()
        .any(|used| used.role() == SourceUseRole::Attempted)
}

fn consult(
    uses: &mut Vec<SourceUse>,
    fact: &OriginalValueFact<'_, '_, '_>,
) -> Result<(), CatalogError> {
    let role = if matches!(fact.value(), Value::Error(_)) {
        SourceUseRole::Attempted
    } else {
        SourceUseRole::Selector
    };
    retain(uses, fact.observe(OriginalValuePart::Value, role)?)
}

fn retain(uses: &mut Vec<SourceUse>, source: SourceUse) -> Result<(), CatalogError> {
    uses.try_reserve(1)
        .map_err(CatalogError::SourceUseAllocation)?;
    uses.push(source);
    Ok(())
}
