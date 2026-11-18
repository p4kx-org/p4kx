// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::{
    FieldOrigin, Guid, SourceUse, SourceUseRole, SourceValue, SourcedValue, Value,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{
    OriginalDatum, OriginalFlowFields, OriginalValueFact, OriginalValuePart,
};
use crate::catalog_v3::tag_catalog::dependent_origins::ContextualSourcedValue;
use crate::catalog_v3::tag_catalog::{
    CapturedTags, ContextualTagPath, OriginalTagPath, TagDatum, TagPathLookup,
};
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

type Fact<'scope, 'read, 'source> = OriginalValueFact<'scope, 'read, 'source>;
type TagList = SourcedValue<Vec<MissionLocationTag>>;

/// One original operand and the source evidence of its projected Tag path.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct MissionLocationTag {
    path: ContextualTagPath,
    dangling_guid: Option<SourcedValue<CatalogGuid>>,
}

impl MissionLocationTag {
    /// The projected path and its occurrence-local and shared path evidence.
    ///
    /// # Errors
    /// Returns an accounting error if the bound source lineage is inconsistent.
    pub fn path(&self) -> Result<Cow<'_, SourcedValue<OriginalTagPath>>, CatalogError> {
        self.path.sourced()
    }

    /// Borrows the optional original dangling-GUID cell without collapsing its source state.
    #[must_use]
    pub const fn dangling_guid_source(&self) -> Option<&SourcedValue<CatalogGuid>> {
        self.dangling_guid.as_ref()
    }

    /// The authored target GUID for a dangling operand.
    #[must_use]
    pub fn dangling_guid(&self) -> Option<(Guid, &FieldOrigin)> {
        let datum = self.dangling_guid.as_ref()?;
        Some((datum.value()?.0, datum.origin()))
    }
}

/// The positive and negative operands of one authored mission location term.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct MissionLocationTagTerm {
    /// The positive operands in source order.
    pub positive_tags: SourcedValue<Vec<MissionLocationTag>>,
    /// The negative operands in source order.
    pub negative_tags: SourcedValue<Vec<MissionLocationTag>>,
}

pub(crate) struct LocationTags {
    pub(crate) tags: TagList,
    pub(crate) produces: SourcedValue<MissionLocationTagTerm>,
    pub(crate) consumes: SourcedValue<MissionLocationTagTerm>,
    pub(crate) ai_spawn: TagList,
}

pub(crate) struct Reader<'context, 'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    tags: &'context CapturedTags<'context, 'source>,
}

#[derive(Default)]
struct SelectionInputs {
    uses: Vec<SourceUse>,
    failures: Vec<SourceUse>,
}

impl<'context, 'fields, 'scope, 'read, 'source> Reader<'context, 'fields, 'scope, 'read, 'source> {
    pub(crate) const fn new(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        tags: &'context CapturedTags<'context, 'source>,
    ) -> Self {
        Self { fields, tags }
    }

    /// Selects the first readable Struct and retains its authored prefix decisions.
    pub(crate) fn original_owner(
        &self,
        members: &[&str],
    ) -> Result<SourcedValue<&'fields Fact<'scope, 'read, 'source>>, CatalogError> {
        self.read(None, members, &[], |fact, inputs| {
            let (owner, complete) = select_owner(self.fields, fact, inputs)?;
            let state = owner.map_or_else(|| non_present(fact.value()), SourceValue::Present);
            Ok((state, complete))
        })
    }

    /// Selects one original field without losing null or unsupported value shapes.
    pub(crate) fn original_value(
        &self,
        members: &[&str],
    ) -> Result<SourcedValue<&'fields Fact<'scope, 'read, 'source>>, CatalogError> {
        self.read(None, members, &[], |fact, _| {
            Ok((SourceValue::Present(fact), true))
        })
    }

    pub(crate) fn location(&self) -> Result<LocationTags, CatalogError> {
        Ok(LocationTags {
            tags: self.array(None, &["locationData", "generalTags", "tags"], &[])?,
            produces: self.term("producesTags")?,
            consumes: self.term("consumesTags")?,
            ai_spawn: self.array(None, &["locationData", "aiSpawnTags", "tags"], &[])?,
        })
    }

    pub(crate) fn variant_failures(&self) -> Result<Vec<SourceUse>, CatalogError> {
        let mut inputs = SelectionInputs::default();
        self.select(
            None,
            &["locationData", "stringVariants", "variants"],
            &mut inputs,
            |_, _| Ok((SourceValue::Present(()), true)),
        )?;
        Ok(inputs.failures)
    }

    pub(crate) fn variant(
        &self,
        owner: &'fields Fact<'scope, 'read, 'source>,
        failures: &[SourceUse],
    ) -> Result<MissionLocationTag, CatalogError> {
        if let Some((_, fact)) = self.fields.field(Some(owner), "tag")? {
            return self.operand(fact, failures);
        }
        let mut uses = failures.to_vec();
        uses.push(self.fields.survey(Some(owner))?);
        Ok(MissionLocationTag {
            path: ContextualSourcedValue::immediate(self.fields.sources().value(
                SourceValue::Absent,
                &uses,
                failures.is_empty(),
            )),
            dangling_guid: None,
        })
    }

    fn array(
        &self,
        owner: Option<&'fields Fact<'scope, 'read, 'source>>,
        members: &[&str],
        failures: &[SourceUse],
    ) -> Result<TagList, CatalogError> {
        self.read(owner, members, failures, |fact, inputs| {
            let failures = &inputs.failures;
            let value = match fact.value() {
                Value::Array(values) => SourceValue::Present(
                    values
                        .iter()
                        .map(|value| {
                            self.fields
                                .fact(value)
                                .and_then(|fact| self.operand(fact, failures))
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                other => non_present(other),
            };
            Ok((value, true))
        })
    }

    fn term(&self, name: &str) -> Result<SourcedValue<MissionLocationTagTerm>, CatalogError> {
        self.read(None, &["locationData", name], &[], |fact, inputs| {
            let (selected, complete) = select_owner(self.fields, fact, inputs)?;
            let failures = &inputs.failures;
            let value = if let Some(owner) = selected {
                SourceValue::Present(MissionLocationTagTerm {
                    positive_tags: self.array(Some(owner), &["positiveTags"], failures)?,
                    negative_tags: self.array(Some(owner), &["negativeTags"], failures)?,
                })
            } else {
                non_present(fact.value())
            };
            Ok((value, complete))
        })
    }

    fn read<T>(
        &self,
        owner: Option<&'fields Fact<'scope, 'read, 'source>>,
        members: &[&str],
        failures: &[SourceUse],
        decode: impl FnOnce(
            &'fields Fact<'scope, 'read, 'source>,
            &mut SelectionInputs,
        ) -> Result<(SourceValue<T>, bool), CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let mut inputs = SelectionInputs {
            uses: failures.to_vec(),
            failures: failures.to_vec(),
        };
        let (state, complete) = self.select(owner, members, &mut inputs, decode)?;
        let complete = complete && failures.is_empty();
        Ok(self.fields.sources().value(state, &inputs.uses, complete))
    }

    fn select<T>(
        &self,
        owner: Option<&'fields Fact<'scope, 'read, 'source>>,
        members: &[&str],
        inputs: &mut SelectionInputs,
        decode: impl FnOnce(
            &'fields Fact<'scope, 'read, 'source>,
            &mut SelectionInputs,
        ) -> Result<(SourceValue<T>, bool), CatalogError>,
    ) -> Result<(SourceValue<T>, bool), CatalogError> {
        let (name, remaining) = members
            .split_first()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let Some((_, fact)) = self.fields.field(owner, name)? else {
            inputs.uses.push(self.fields.survey(owner)?);
            return Ok((SourceValue::Absent, true));
        };
        inputs
            .uses
            .push(fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
        if remaining.is_empty() {
            let (state, complete) = decode(fact, inputs)?;
            return field_value(fact, &mut inputs.uses, state, complete);
        }
        let (selected, complete) = select_owner(self.fields, fact, inputs)?;
        let (state, child_complete) = match selected {
            Some(selected) => self.select(Some(selected), remaining, inputs, decode)?,
            None => field_value(fact, &mut inputs.uses, non_present(fact.value()), true)?,
        };
        Ok((state, complete && child_complete))
    }

    fn operand(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        failures: &[SourceUse],
    ) -> Result<MissionLocationTag, CatalogError> {
        if matches!(fact.value(), Value::Reference(_)) {
            let reference = fact.reference_observation()?;
            let path = match reference.tag_path(self.tags)? {
                TagPathLookup::Ready { value, .. } => self.bind_tag(value, failures)?,
                TagPathLookup::Unavailable { value } => {
                    let (_, mut inputs) = value.split();
                    let guid = reference.guid()?;
                    let SourceValue::Present(guid_value) = guid.value() else {
                        return Err(ExportAccountingError::ConflictingSourceDatum.into());
                    };
                    inputs.retain(guid.contributions().0.iter().cloned(), false);
                    inputs.retain(failures.iter().cloned(), failures.is_empty());
                    inputs.bind(
                        SourceValue::Present(OriginalTagPath::fallback(CatalogGuid(*guid_value))),
                        self.fields.sources(),
                    )?
                }
            };
            return Ok(MissionLocationTag {
                path,
                dangling_guid: None,
            });
        }
        if let Some(guid) = fact.dangling_guid(SourceUseRole::Payload)? {
            let Some(TagPathLookup::Ready { value, .. }) = fact.dangling_tag()? else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            return Ok(MissionLocationTag {
                path: self.bind_tag(value, failures)?,
                dangling_guid: Some(self.bind_local(guid.map(CatalogGuid)?, failures)),
            });
        }
        let datum = fact.bind(
            OriginalValuePart::Value,
            non_present(fact.value()),
            SourceUseRole::Selector,
            true,
        )?;
        Ok(MissionLocationTag {
            path: ContextualSourcedValue::immediate(self.bind_local(datum, failures)),
            dangling_guid: None,
        })
    }

    fn bind_tag(
        &self,
        datum: TagDatum,
        failures: &[SourceUse],
    ) -> Result<ContextualTagPath, CatalogError> {
        let (value, mut inputs) = datum.split();
        inputs.retain(failures.iter().cloned(), failures.is_empty());
        inputs.bind(value, self.fields.sources())
    }

    fn bind_local<T>(&self, datum: OriginalDatum<T>, failures: &[SourceUse]) -> SourcedValue<T> {
        let (value, mut inputs, complete) = datum.into_parts();
        inputs.extend(failures.iter().cloned());
        self.fields
            .sources()
            .value(value, &inputs, complete && failures.is_empty())
    }
}

fn select_owner<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    fact: &'fields Fact<'scope, 'read, 'source>,
    inputs: &mut SelectionInputs,
) -> Result<(Option<&'fields Fact<'scope, 'read, 'source>>, bool), CatalogError> {
    if matches!(fact.value(), Value::Struct(_) | Value::Array(_)) {
        inputs
            .uses
            .push(fact.observe(OriginalValuePart::Value, SourceUseRole::Selector)?);
    }
    let mut complete = true;
    let owner = fields.first_struct_observed(fact, |item| {
        let fact = fields.fact(item)?;
        let selector = fact.observe(OriginalValuePart::Value, SourceUseRole::Selector)?;
        if matches!(item, Value::Error(_)) {
            complete = false;
            let attempt = fact.observe(OriginalValuePart::Value, SourceUseRole::Attempted)?;
            inputs.failures.extend([selector.clone(), attempt.clone()]);
            inputs.uses.push(attempt);
        }
        inputs.uses.push(selector);
        Ok(())
    })?;
    Ok((owner, complete))
}

fn field_value<T>(
    fact: &Fact<'_, '_, '_>,
    uses: &mut Vec<SourceUse>,
    state: SourceValue<T>,
    complete: bool,
) -> Result<(SourceValue<T>, bool), CatalogError> {
    let datum = fact.bind(
        OriginalValuePart::Value,
        state,
        SourceUseRole::Selector,
        complete,
    )?;
    let (state, inputs, complete) = datum.into_parts();
    uses.extend(inputs);
    Ok((state, complete))
}

const fn non_present<T>(value: &Value<'_>) -> SourceValue<T> {
    match value {
        Value::Null => SourceValue::Null,
        Value::Error(_) => SourceValue::Unreadable,
        _ => SourceValue::Unsupported,
    }
}
