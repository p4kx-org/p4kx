// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::OriginalValueFact;
use crate::mission_index::MissionIndex;
use crate::resource_integer::ResourceInt64;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;
use crate::template_reference_index::TemplateReferences;
use crate::template_source_members::TemplateMembers;
use crate::template_source_refs::DeclarationBinding;

/// One selected result and the projected amounts of its original entries.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BrokerReputationResult {
    pub(crate) source: SourceValueRef,
    pub(crate) amounts: Vec<BrokerReputationAmount>,
}

/// One actual result entry, with its independently sourced derived reward amount.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BrokerReputationAmount {
    pub(crate) source: SourceValueRef,
    pub(crate) reward: DeclarationBinding,
    pub(crate) amount: Option<SourcedValue<ResourceInt64>>,
}

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    input: &OriginalCatalogInput<'_, '_>,
    index: &MissionIndex<'source>,
) -> Result<Vec<BrokerReputationResult>, CatalogError> {
    let fields = members.fields();
    let Some((_, results)) = fields.field(None, "missionResultReputationRewards")? else {
        return Ok(Vec::new());
    };
    let mut rows = Vec::new();
    for result in fields.structs(results)? {
        fields.check_cancel()?;
        let amounts = match fields.field(Some(result), "reputationAmounts")? {
            Some((_, declaration)) => fields
                .structs(declaration)?
                .into_iter()
                .map(|entry| amount(members, references, input, index, entry))
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };
        rows.push(BrokerReputationResult {
            source: members.value_ref(result)?,
            amounts,
        });
    }
    Ok(rows)
}

fn amount<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    input: &OriginalCatalogInput<'_, '_>,
    index: &MissionIndex<'source>,
    entry: &'fields OriginalValueFact<'scope, 'read, 'source>,
) -> Result<BrokerReputationAmount, CatalogError> {
    let fields = members.fields();
    let reward = crate::template_declaration_binding::field(members, Some(entry), "reward")?;
    let amount = match fields.field(Some(entry), "reward")? {
        Some((_, fact)) if matches!(fact.value(), Value::Reference(_)) => {
            index.reward_observed(input, references.observation(fact)?)?
        }
        _ => None,
    };
    Ok(BrokerReputationAmount {
        source: members.value_ref(entry)?,
        reward,
        amount,
    })
}
