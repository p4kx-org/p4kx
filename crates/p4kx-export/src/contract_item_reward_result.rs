// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceUseRole, SourceValue, SourcedValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_integer::ResourceInt64;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog;
use crate::source_value_catalog::{
    SourcePayload, SourceRecordReference, SourceValueRef, SourceWeakReference,
};

use super::value_ref;

#[path = "contract_item_reward_links.rs"]
pub(crate) mod links;

const RESULT: &str = "ContractResult_Item";

/// The original settings of one physical item result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ContractItemRewardResult<'source> {
    source: SourceValueRef,
    mission_results: SourcedValue<Vec<SourcedValue<bool>>>,
    #[schemars(with = "SourcedValue<Box<CatalogRecordReference>>")]
    entity_class: SourcedValue<Box<SourceRecordReference<'source>>>,
    amount: SourcedValue<ResourceInt64>,
    award_only_to_mission_owner: SourcedValue<bool>,
    send_to_player_home_location: SourcedValue<bool>,
    target_location: SourcedValue<ItemRewardTarget<'source>>,
    notification: SourcedValue<SourceValueRef>,
}

/// An authored location selector and its exact joined value when it targets an owned instance.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ItemRewardTarget<'source> {
    reference: Box<SourceWeakReference<'source>>,
    target_value: Option<SourceValueRef>,
}

pub(crate) fn read<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
) -> Result<Vec<ContractItemRewardResult<'source>>, CatalogError> {
    let mut rows = Vec::new();
    for fact in fields.facts() {
        fields.check_cancel()?;
        if fact.field().name() == "contractResults" && is_item(fact.value()) {
            rows.try_reserve(1)
                .map_err(CatalogError::SourceUseAllocation)?;
            rows.push(
                Reader {
                    fields,
                    owner: fact,
                }
                .read()?,
            );
        }
    }
    Ok(rows)
}

fn is_item(value: &Value<'_>) -> bool {
    matches!(value, Value::Struct(result) if result.type_name() == RESULT)
}

type Fact<'fields, 'scope, 'read, 'source> = &'fields OriginalValueFact<'scope, 'read, 'source>;

#[derive(Clone, Copy)]
enum MemberUse {
    Payload,
    WeakPointerSelector,
}

struct Reader<'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    owner: Fact<'fields, 'scope, 'read, 'source>,
}

impl<'fields, 'scope, 'read, 'source> Reader<'fields, 'scope, 'read, 'source> {
    fn read(&self) -> Result<ContractItemRewardResult<'source>, CatalogError> {
        Ok(ContractItemRewardResult {
            source: value_ref(self.owner),
            mission_results: self.member("missionResults", |fact| self.outcomes(fact))?,
            entity_class: self.member("entityClass", |fact| entity_reference(self.fields, fact))?,
            amount: self.member("amount", |fact| {
                Ok(match fact.value() {
                    Value::Int32(value) => {
                        SourceValue::Present(ResourceInt64::new(i64::from(*value)))
                    }
                    _ => SourceValue::Unsupported,
                })
            })?,
            award_only_to_mission_owner: self
                .member("awardOnlyToMissionOwner", |fact| Ok(flag(fact)))?,
            send_to_player_home_location: self
                .member("sendToPlayerHomeLocation", |fact| Ok(flag(fact)))?,
            target_location: self.member_with_use(
                "targetLocation",
                MemberUse::WeakPointerSelector,
                |fact| target_selector(self.fields, fact),
            )?,
            notification: self.member("notification", |fact| {
                Ok(match fact.value() {
                    Value::Struct(_) => SourceValue::Present(value_ref(fact)),
                    _ => SourceValue::Unsupported,
                })
            })?,
        })
    }

    fn member<T>(
        &self,
        name: &str,
        read: impl FnOnce(Fact<'fields, 'scope, 'read, 'source>) -> Result<SourceValue<T>, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.member_with_use(name, MemberUse::Payload, read)
    }

    fn member_with_use<T>(
        &self,
        name: &str,
        member_use: MemberUse,
        read: impl FnOnce(Fact<'fields, 'scope, 'read, 'source>) -> Result<SourceValue<T>, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some((_, fact)) = self.fields.field(Some(self.owner), name)? else {
            let survey = self.fields.survey(Some(self.owner))?;
            return Ok(self
                .fields
                .sources()
                .value(SourceValue::Absent, &[survey], true));
        };
        cell(self.fields, fact, member_use, read)
    }

    fn outcomes(
        &self,
        fact: Fact<'fields, 'scope, 'read, 'source>,
    ) -> Result<SourceValue<Vec<SourcedValue<bool>>>, CatalogError> {
        match fact.value() {
            Value::Array(_) => self
                .fields
                .elements(fact)?
                .into_iter()
                .map(|item| self.outcome(item))
                .collect::<Result<Vec<_>, _>>()
                .map(SourceValue::Present),
            Value::Struct(_) => Ok(SourceValue::Present(vec![self.outcome(fact)?])),
            _ => Ok(SourceValue::Unsupported),
        }
    }

    fn outcome(
        &self,
        item: Fact<'fields, 'scope, 'read, 'source>,
    ) -> Result<SourcedValue<bool>, CatalogError> {
        self.fields.check_cancel()?;
        if matches!(item.value(), Value::Struct(_)) {
            let Some((_, fact)) = self.fields.field(Some(item), "value")? else {
                let survey = self.fields.survey(Some(item))?;
                return Ok(self
                    .fields
                    .sources()
                    .value(SourceValue::Absent, &[survey], true));
            };
            return cell(self.fields, fact, MemberUse::Payload, |fact| Ok(flag(fact)));
        }
        cell(self.fields, item, MemberUse::Payload, |fact| Ok(flag(fact)))
    }
}

fn cell<'fields, 'scope, 'read, 'source, T>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    fact: Fact<'fields, 'scope, 'read, 'source>,
    member_use: MemberUse,
    read: impl FnOnce(Fact<'fields, 'scope, 'read, 'source>) -> Result<SourceValue<T>, CatalogError>,
) -> Result<SourcedValue<T>, CatalogError> {
    let state = match fact.value() {
        Value::Null => SourceValue::Null,
        Value::Error(_) => SourceValue::Unreadable,
        _ => read(fact)?,
    };
    let role = match (member_use, &state) {
        (MemberUse::Payload, SourceValue::Present(_)) => SourceUseRole::Payload,
        _ => SourceUseRole::Selector,
    };
    let declared = fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?;
    let mut bound = fact.bind(OriginalValuePart::Value, state, role, true)?;
    bound.contribute(declared);
    if matches!(member_use, MemberUse::WeakPointerSelector) {
        bound.contribute(fact.observe(OriginalValuePart::DataType, SourceUseRole::Selector)?);
    }
    Ok(bound.into_cell(fields.sources()))
}

const fn flag(fact: &OriginalValueFact<'_, '_, '_>) -> SourceValue<bool> {
    match fact.value() {
        Value::Bool(value) => SourceValue::Present(*value),
        _ => SourceValue::Unsupported,
    }
}

fn entity_reference<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    fact: &OriginalValueFact<'scope, 'read, 'source>,
) -> Result<SourceValue<Box<SourceRecordReference<'source>>>, CatalogError> {
    let Value::Reference(reference) = fact.value() else {
        return Ok(SourceValue::Unsupported);
    };
    if reference.location().is_some() || reference.type_name() != "EntityClassDefinition" {
        return Ok(SourceValue::Invalid);
    }
    let payload = source_value_catalog::read_reference_payload(
        fact.reference_observation()?,
        fields.sources(),
    )?;
    Ok(match payload {
        SourcePayload::RecordReference { value } => SourceValue::Present(value),
        SourcePayload::WeakReference { value } => (*value)
            .into_record()
            .map_or(SourceValue::Invalid, |record| {
                SourceValue::Present(Box::new(record))
            }),
        _ => SourceValue::Unsupported,
    })
}

fn target_selector<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    fact: &OriginalValueFact<'scope, 'read, 'source>,
) -> Result<SourceValue<ItemRewardTarget<'source>>, CatalogError> {
    if fact.field().storage().data_type() != DataType::WeakPointer {
        return Ok(SourceValue::Unsupported);
    }
    let Value::Reference(_) = fact.value() else {
        return Ok(SourceValue::Invalid);
    };
    let observed = fact.reference_observation()?;
    let target = fields.target_fact(&observed)?.map(value_ref);
    let payload = source_value_catalog::read_reference_payload(observed, fields.sources())?;
    Ok(match payload {
        SourcePayload::WeakReference { value } => SourceValue::Present(ItemRewardTarget {
            reference: value,
            target_value: target,
        }),
        _ => SourceValue::Unsupported,
    })
}
