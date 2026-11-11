// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashSet;

use p4kx_extraction::Value;

use crate::CatalogError;
use crate::broker_canonical_read::BrokerEmission;
use crate::broker_entry::BrokerProjection;
use crate::mission_flow_owner::{FlowOwnerKind, MissionFlowOwnerRef};
use crate::mission_index::MissionIndex;
use crate::record_read::ReadError;
use crate::source_value_catalog::SourceValueRef;
use crate::template_context_read::{
    self, ContextEmission, ContextFields, ContextPlan, ContextSources, PreparedSpawns,
};
use crate::template_property_kind::PropertyValueKind;
use crate::template_reference_index::TemplateReferences;
use crate::template_source_members::TemplateMembers;
use crate::template_spawn_read::{SpawnEmission, TemplateSpawns};
use crate::template_tag_cache::TemplateTagCache;
use crate::template_value_meaning::{TemplateValueMeaning, TemplateValueStatus};
use crate::template_value_meaning_read::CheckedValue;

const FAMILY: FlowOwnerKind = FlowOwnerKind::MissionBrokerEntry;

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &mut RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
    tags: &TemplateTagCache<'_, 'fields, 'scope, 'read, 'source>,
    sources: &ContextSources<'_, '_, 'source>,
    plan: &ContextPlan<'_, 'fields, 'scope, 'read, 'source>,
    spawns: TemplateSpawns<'fields, 'scope, 'read, 'source>,
    index: &MissionIndex<'source>,
) -> Result<BrokerEmission<'source>, ReadError> {
    let mut vocabularies =
        crate::template_vocabulary_read::broker_prefix(members, sources.unknowns)?;
    let standing_requirements =
        crate::broker_standing_read::read(members, sources.unknowns, sources.capture)?;
    let (values, spawns) = prepare_spawns(members, references, tags, sources, plan, spawns)?;
    let modifiers = crate::template_modifier_projection::required(
        members,
        sources.capture,
        sources.unknowns,
        &mut vocabularies,
    )?;
    let ContextEmission {
        mut context,
        pools,
        ledger,
        failures,
    } = template_context_read::emit(
        members,
        references,
        tags,
        sources,
        plan,
        ContextFields {
            spawns,
            values,
            modifiers,
        },
        FAMILY,
    )?;
    vocabularies.append(&mut context.vocabularies);
    context.vocabularies = vocabularies;
    let reputation_results = crate::broker_reputation_read::read(
        members,
        references.get_mut(),
        sources.projection.original(),
        index,
    )?;
    let title = title(members)?;
    let projection = BrokerProjection {
        broker: MissionFlowOwnerRef::read(members.fields(), FAMILY)?,
        title,
        context,
        standing_requirements,
        reputation_results,
    };
    Ok(BrokerEmission {
        projection,
        pools,
        ledger,
        failures,
    })
}

fn prepare_spawns<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
    tags: &TemplateTagCache<'_, 'fields, 'scope, 'read, 'source>,
    sources: &ContextSources<'_, '_, 'source>,
    plan: &ContextPlan<'_, 'fields, 'scope, 'read, 'source>,
    mut spawns: TemplateSpawns<'fields, 'scope, 'read, 'source>,
) -> Result<(Vec<TemplateValueMeaning>, PreparedSpawns<'source>), ReadError> {
    let values = properties(members, references, tags, sources, plan, &mut spawns)?;
    let bindings = crate::template_pool_binding_read::read(members, plan.spawns, &spawns, FAMILY)?;
    let SpawnEmission {
        projections,
        pools,
        failures,
    } = spawns.emit(members)?;
    if let Some((_, error)) = failures.into_iter().next() {
        return Err(error);
    }
    let spawns = PreparedSpawns {
        bindings,
        projections,
        pools,
    };
    Ok((values, spawns))
}

fn title(
    members: &TemplateMembers<'_, '_, '_, '_>,
) -> Result<Option<SourceValueRef>, CatalogError> {
    members
        .fields()
        .field(None, "title")?
        .map(|(_, fact)| members.value_ref(fact))
        .transpose()
}

fn properties<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
    tags: &TemplateTagCache<'_, 'fields, 'scope, 'read, 'source>,
    sources: &ContextSources<'_, '_, 'source>,
    plan: &ContextPlan<'_, 'fields, 'scope, 'read, 'source>,
    spawns: &mut TemplateSpawns<'fields, 'scope, 'read, 'source>,
) -> Result<Vec<TemplateValueMeaning>, ReadError> {
    let mut rows = Vec::new();
    let mut seen = HashSet::new();
    for fact in plan.properties.iter().filter_map(|property| property.value) {
        members.fields().check_cancel()?;
        if !seen.insert(fact.ordinal()) {
            continue;
        }
        if matches!(fact.value(), Value::Struct(source) if
            PropertyValueKind::read(source.type_name()).is_some_and(PropertyValueKind::is_spawn))
        {
            spawns.required(references, tags, sources.tree, fact, sources.capture)?;
        }
        let CheckedValue { row, failure } = crate::template_value_meaning_read::one(
            members,
            fact,
            TemplateValueStatus::Recognized,
            sources.capture,
        )?;
        if let Some(error) = failure {
            return Err(error);
        }
        rows.push(row);
    }
    Ok(rows)
}
