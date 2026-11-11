// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue, StructValue, Value};

use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::mission_facts::choice;
use crate::mission_layers;
use crate::record_fields;
use crate::resource_integer::ResourceInt64;
use crate::{CatalogError, ExportAccountingError};

use super::FixedReward;

const RESULT_BLOCK: &str = "contractResults";
const REWARD_RESULT: &str = "ContractResult_Reward";

/// Reads the parent contract's successful fixed payouts and their original source decisions.
pub(crate) fn fixed_rewards<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    contract: &StructValue<'source>,
) -> Result<SourcedValue<Vec<FixedReward<'source>>>, CatalogError> {
    let reader = Reader { fields };
    let owner = fields
        .owner_of(contract)?
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let mut path = Vec::new();
    let block =
        fields.observe_struct_selection(owner, RESULT_BLOCK, |used| retain(&mut path, used))?;
    let results = match block {
        Some(block) => fields
            .observe_field_selection(block, RESULT_BLOCK, &mut |used| retain(&mut path, used))?,
        None => None,
    };
    let mut membership = Vec::new();
    choice::extend(&mut membership, &path)?;
    let mut rewards = Vec::new();
    if let Some(results) = results {
        for value in mission_layers::items_value(results.value()) {
            let candidate = fields.fact(value)?;
            let mut branch = Vec::new();
            let reward = reader.selected_reward(candidate, &mut branch)?;
            choice::extend(&mut membership, &branch)?;
            if let Some(reward) = reward {
                choice::extend(&mut branch, &path)?;
                rewards
                    .try_reserve(1)
                    .map_err(CatalogError::SourceUseAllocation)?;
                rewards.push(reader.payout(reward, &mut branch)?);
            }
        }
    }
    Ok(reader.bind(SourceValue::Present(rewards), &membership))
}

type Fact<'fields, 'scope, 'read, 'source> = &'fields OriginalValueFact<'scope, 'read, 'source>;

struct Reader<'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
}

impl<'fields, 'scope, 'read, 'source> Reader<'fields, 'scope, 'read, 'source> {
    fn selected_reward(
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
        if result.type_name() != REWARD_RESULT
            || !self
                .fields
                .first_mission_outcome(candidate, |used| retain(uses, used))?
        {
            return Ok(None);
        }
        self.fields
            .observe_struct_selection(candidate, "contractReward", |used| retain(uses, used))
    }

    fn payout(
        &self,
        reward: Fact<'fields, 'scope, 'read, 'source>,
        path: &mut Vec<SourceUse>,
    ) -> Result<FixedReward<'source>, CatalogError> {
        let prefix = path.len();
        let currency = self.member(reward, "currencyType", path, record_fields::text_value)?;
        path.truncate(prefix);
        let amount = self.member(reward, "reward", path, exact_integer)?;
        path.truncate(prefix);
        let max = self.member(reward, "max", path, exact_integer)?;
        path.truncate(prefix);
        let plus_bonuses =
            self.member(reward, "plusBonuses", path, record_fields::boolean_value)?;
        Ok(FixedReward {
            currency,
            amount,
            max,
            plus_bonuses,
        })
    }

    fn member<T>(
        &self,
        owner: Fact<'fields, 'scope, 'read, 'source>,
        name: &str,
        uses: &mut Vec<SourceUse>,
        decode: impl FnOnce(&Value<'source>) -> Option<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some(fact) = self
            .fields
            .observe_field_declaration(owner, name, &mut |used| retain(uses, used))?
        else {
            return Ok(self.bind(SourceValue::Absent, uses));
        };
        let state = match fact.value() {
            Value::Error(_) => {
                consult(uses, fact)?;
                SourceValue::Unreadable
            }
            Value::Null => {
                consult(uses, fact)?;
                SourceValue::Null
            }
            value => {
                if let Some(value) = decode(value) {
                    retain(
                        uses,
                        fact.observe(OriginalValuePart::Value, SourceUseRole::Payload)?,
                    )?;
                    SourceValue::Present(value)
                } else {
                    consult(uses, fact)?;
                    SourceValue::Invalid
                }
            }
        };
        Ok(self.bind(state, uses))
    }

    fn bind<T>(&self, value: SourceValue<T>, uses: &[SourceUse]) -> SourcedValue<T> {
        let complete = !uses
            .iter()
            .any(|used| used.role() == SourceUseRole::Attempted);
        self.fields.sources().value(value, uses, complete)
    }
}

fn exact_integer(value: &Value<'_>) -> Option<ResourceInt64> {
    record_fields::integer_value(value).map(ResourceInt64::new)
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
