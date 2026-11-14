// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashMap;

use p4kx_extraction::RecordTree;

use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::export_ledger::ExportLedger;
use crate::mission_flow_owner::FlowOwnerKind;
use crate::source_fields::FieldCapture;
use crate::spawn_pools::SpawnPools;
use crate::template_modifier_projection::TemplateModifierProjection;
use crate::template_objective_plan::TemplateObjectiveSelection;
use crate::template_output_plan::TemplateOutputSelection;
use crate::template_plan::TemplatePlan;
use crate::template_projection::{MissionContext, TemplateOwnerProjection, TemplateProjection};
use crate::template_property_plan::SelectedProperty;
use crate::template_reference_index::TemplateReferences;
use crate::template_source_members::TemplateMembers;
use crate::template_spawn_projection::{TemplatePoolBinding, TemplateSpawnProjection};
use crate::template_spawn_read::SpawnEmission;
use crate::template_tag_cache::TemplateTagCache;
use crate::template_value_meaning::TemplateValueMeaning;
use crate::template_weak_plan::TemplateWeakSelection;
use crate::{CatalogError, CatalogReadFailure};

pub(crate) struct TemplateContext<'source> {
    pub(crate) projection: TemplateProjection<'source>,
    pub(crate) pools: SpawnPools<'source>,
    pub(crate) errors: crate::template_derived_failures::TemplateErrors,
    pub(crate) ledger: ExportLedger,
    pub(crate) failures: Vec<CatalogReadFailure<'source>>,
}

pub(crate) struct ContextPlan<'plan, 'fields, 'scope, 'read, 'source> {
    pub(crate) objectives: &'plan [TemplateObjectiveSelection<'fields, 'scope, 'read, 'source>],
    pub(crate) properties: &'plan [SelectedProperty<'fields, 'scope, 'read, 'source>],
    pub(crate) weak: &'plan [TemplateWeakSelection<'fields, 'scope, 'read, 'source>],
    pub(crate) outputs: &'plan [TemplateOutputSelection<'fields, 'scope, 'read, 'source>],
    pub(crate) spawns:
        &'plan crate::template_spawn_plan::TemplateSpawnPlan<'fields, 'scope, 'read, 'source>,
}

impl<'plan, 'fields, 'scope, 'read, 'source> ContextPlan<'plan, 'fields, 'scope, 'read, 'source> {
    pub(crate) const fn from_plan(
        plan: &'plan TemplatePlan<'fields, 'scope, 'read, 'source>,
    ) -> Self {
        Self {
            objectives: plan.objectives.as_slice(),
            properties: plan.properties.as_slice(),
            weak: plan.weak.as_slice(),
            outputs: plan.outputs.as_slice(),
            spawns: &plan.spawns,
        }
    }
}

pub(crate) struct ContextSources<'context, 'catalog, 'source> {
    pub(crate) tags: &'context CapturedTags<'catalog, 'source>,
    pub(crate) tree: &'context RecordTree<'source>,
    pub(crate) projection: &'context CatalogProjection<'catalog, 'source>,
    pub(crate) unknowns: &'context crate::template_issues::Unknowns,
    pub(crate) capture: &'context dyn FieldCapture<'source>,
}

/// The actual one-pass spawn reads and the contextual links they selected.
pub(crate) struct PreparedSpawns<'source> {
    pub(crate) bindings: Vec<TemplatePoolBinding>,
    pub(crate) projections: Vec<TemplateSpawnProjection>,
    pub(crate) pools: SpawnPools<'source>,
}

struct SpawnPreparation<'source> {
    spawns: PreparedSpawns<'source>,
    failures: Vec<(u64, crate::record_read::ReadError)>,
}

fn prepare_spawns<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
    tags: &TemplateTagCache<'_, 'fields, 'scope, 'read, 'source>,
    sources: &ContextSources<'_, '_, 'source>,
    plan: &crate::template_spawn_plan::TemplateSpawnPlan<'fields, 'scope, 'read, 'source>,
    owner: FlowOwnerKind,
) -> Result<SpawnPreparation<'source>, CatalogError> {
    let spawns = crate::template_spawn_read::TemplateSpawns::read(
        members.fields(),
        references,
        tags,
        sources.tree,
        &plan.values,
        sources.capture,
    )?;
    let bindings = crate::template_pool_binding_read::read(members, plan, &spawns, owner)?;
    let SpawnEmission {
        projections,
        pools,
        failures,
    } = spawns.emit(members)?;
    Ok(SpawnPreparation {
        spawns: PreparedSpawns {
            bindings,
            projections,
            pools,
        },
        failures,
    })
}

pub(crate) struct ContextFields<'source> {
    pub(crate) spawns: PreparedSpawns<'source>,
    pub(crate) values: Vec<TemplateValueMeaning>,
    pub(crate) modifiers: Vec<TemplateModifierProjection>,
}

pub(crate) struct ContextEmission<'source> {
    pub(crate) context: MissionContext<'source>,
    pub(crate) pools: SpawnPools<'source>,
    pub(crate) ledger: ExportLedger,
    pub(crate) failures: Vec<CatalogReadFailure<'source>>,
}

pub(crate) fn emit<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &mut RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
    tags: &TemplateTagCache<'_, 'fields, 'scope, 'read, 'source>,
    sources: &ContextSources<'_, '_, 'source>,
    plan: &ContextPlan<'_, 'fields, 'scope, 'read, 'source>,
    fields: ContextFields<'source>,
    kind: FlowOwnerKind,
) -> Result<ContextEmission<'source>, CatalogError> {
    let ContextFields {
        spawns,
        values,
        modifiers,
    } = fields;
    let PreparedSpawns {
        bindings,
        projections,
        pools,
    } = spawns;
    let tag_paths = crate::template_context_tag_read::read(members, references, tags, kind)?;
    let (property_bindings, objective_bindings) =
        crate::template_context_weak_read::read(members, references.get_mut(), plan.weak)?;
    let linked_outputs =
        crate::template_output_read::read(members, references.get_mut(), plan.outputs)?;
    let ContextOwner {
        projection: owner,
        ledger,
        failures,
    } = read_owner(members, references.get_mut(), sources, kind)?;
    let context = MissionContext {
        objectives: crate::template_context_source_read::objectives(members, plan.objectives)?,
        properties: crate::template_property_projection_read::read(
            members,
            plan.properties,
            sources.unknowns,
        )?,
        value_meanings: values,
        localizations: crate::template_context_source_read::localizations(
            members,
            sources.projection,
            plan.objectives,
            plan.properties,
            kind,
        )?,
        tag_paths,
        property_bindings,
        objective_bindings,
        linked_outputs,
        vocabularies: crate::template_vocabulary_read::read(
            members,
            plan.objectives,
            sources.unknowns,
            kind,
        )?,
        modifiers,
        owner,
        hauling_orders: crate::template_hauling_read::read(members, kind)?,
        spawn_pools: projections,
        pool_bindings: bindings,
    };
    Ok(ContextEmission {
        context,
        pools,
        ledger,
        failures,
    })
}

struct ContextOwner<'source> {
    projection: Option<TemplateOwnerProjection<'source>>,
    ledger: ExportLedger,
    failures: Vec<CatalogReadFailure<'source>>,
}

fn read_owner<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    sources: &ContextSources<'_, '_, 'source>,
    kind: FlowOwnerKind,
) -> Result<ContextOwner<'source>, CatalogError> {
    let mut ledger = ExportLedger::default();
    let mut failures = Vec::new();
    let projection = crate::template_context_owner_read::read(
        members,
        references,
        sources.projection.original(),
        &mut ledger,
        &mut failures,
        kind,
    )?;
    Ok(ContextOwner {
        projection,
        ledger,
        failures,
    })
}

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &mut RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
    sources: &ContextSources<'_, '_, 'source>,
    plan: &ContextPlan<'_, 'fields, 'scope, 'read, 'source>,
) -> Result<TemplateContext<'source>, CatalogError> {
    let kind = FlowOwnerKind::ContractTemplate;
    let tags = TemplateTagCache::new(members.fields(), sources.tags);
    let SpawnPreparation {
        spawns,
        failures: mut errors,
    } = prepare_spawns(members, references, &tags, sources, plan.spawns, kind)?;
    let spawn_failures: HashMap<_, _> = errors
        .iter()
        .map(|(ordinal, error)| {
            (
                *ordinal,
                crate::export_source_failure::diagnosis(error.as_ref()).code,
            )
        })
        .collect();
    let meanings = crate::template_value_meaning_read::read(
        members,
        plan.properties,
        &spawn_failures,
        sources.capture,
    )?;
    errors.extend(meanings.failures);
    let mut errors = crate::template_derived_failures::order(members, plan.properties, errors)?;
    let (modifiers, modifier_failures) =
        crate::template_modifier_projection::read(members, sources.capture)?;
    errors.after_flow.extend(modifier_failures);
    let emitted = emit(
        members,
        references,
        &tags,
        sources,
        plan,
        ContextFields {
            spawns,
            values: meanings.rows,
            modifiers,
        },
        kind,
    )?;
    Ok(TemplateContext {
        projection: TemplateProjection {
            template: members.physical(),
            inputs: crate::template_input_read::read(members)?,
            title: crate::template_context_source_read::title(members)?,
            context: emitted.context,
        },
        pools: emitted.pools,
        errors,
        ledger: emitted.ledger,
        failures: emitted.failures,
    })
}
