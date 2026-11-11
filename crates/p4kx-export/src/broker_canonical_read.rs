// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, Record, RecordTree};

use crate::broker_definition::BrokerDefinition;
use crate::broker_entry::BrokerProjection;
use crate::broker_flow_failure::BrokerProjectionFailure;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::export_ledger::ExportLedger;
use crate::mission_flow_owner::FlowOwnerKind;
use crate::mission_index::MissionIndex;
use crate::record_observation::RecordObservation;
use crate::record_read::ReadError;
use crate::spawn_pools::SpawnPools;
use crate::template_configuration::{MissionFlowConfiguration, MissionPropertyConfiguration};
use crate::template_context_read::{ContextPlan, ContextSources};
use crate::template_field_capture::TemplateFieldCapture;
use crate::template_issues::Unknowns;
use crate::template_meanings::{FlowMeanings, PreparedFlow};
use crate::template_plan::TemplatePlan;
use crate::template_reference_index::TemplateReferences;
use crate::template_source_members::TemplateMembers;
use crate::template_spawn_read::TemplateSpawns;
use crate::template_tag_cache::TemplateTagCache;
use crate::{CatalogError, CatalogReadFailure};

const FAMILY: FlowOwnerKind = FlowOwnerKind::MissionBrokerEntry;

pub(crate) struct Sources<'read, 'catalog, 'source> {
    pub(crate) projection: &'read CatalogProjection<'catalog, 'source>,
    pub(crate) tags: &'read CapturedTags<'catalog, 'source>,
    pub(crate) index: &'read MissionIndex<'source>,
    pub(crate) unknowns: &'read Unknowns,
}

pub(crate) struct BrokerEmission<'source> {
    pub(crate) projection: BrokerProjection<'source>,
    pub(crate) pools: SpawnPools<'source>,
    pub(crate) ledger: ExportLedger,
    pub(crate) failures: Vec<CatalogReadFailure<'source>>,
}

pub(crate) struct CanonicalBroker<'source> {
    pub(crate) definition: BrokerDefinition<'source>,
    pub(crate) flows: Vec<MissionFlowConfiguration>,
    pub(crate) properties: Vec<MissionPropertyConfiguration>,
    pub(crate) projection: Result<BrokerEmission<'source>, Box<BrokerProjectionFailure<'source>>>,
}

pub(crate) fn read<'source>(
    sources: &Sources<'_, '_, 'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    cancel: Option<&CancelToken>,
) -> Result<CanonicalBroker<'source>, CatalogError> {
    let input = sources.projection.original();
    let scope = input.record_scope(record, tree)?;
    let fields = OriginalFlowFields::read(&scope, cancel)?;
    let admission = RecordObservation::capture(input, record)?;
    let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
    let mut references = RefCell::new(TemplateReferences::new(&fields));
    let plan = TemplatePlan::read(&fields, references.get_mut(), FAMILY)?;
    let members = TemplateMembers::new(&fields, plan.roots())?;
    let environment = ContextSources {
        tags: sources.tags,
        tree,
        projection: sources.projection,
        unknowns: sources.unknowns,
        capture: &capture,
    };
    let context = ContextPlan::from_plan(&plan);
    let tags = TemplateTagCache::new(&fields, sources.tags);
    let mut spawns = TemplateSpawns::new(&fields);
    let root = root_spawns(&mut spawns, &references, &tags, &environment, &context)
        .err()
        .map(|error| failed(&admission, error))
        .transpose()?;
    let PreparedFlow { meanings, failure } = crate::template_meanings::prepare(
        &members,
        &plan.flow,
        references.get_mut(),
        root.is_none().then_some(sources.unknowns),
        &capture,
    )?;
    let projected = match (root, failure) {
        (Some(error), _) => Err(error),
        (None, Some(error)) => Err(Box::new(BrokerProjectionFailure::read(
            &admission,
            error.as_ref(),
        )?)),
        (None, None) => match crate::broker_projection_read::read(
            &members,
            &mut references,
            &tags,
            &environment,
            &context,
            spawns,
            sources.index,
        ) {
            Ok(projection) => Ok(projection),
            Err(error) => Err(failed(&admission, error)?),
        },
    };
    emit(
        input,
        tree,
        references.get_mut(),
        &plan,
        meanings,
        projected,
    )
}

fn emit<'fields, 'scope, 'read, 'source>(
    input: &OriginalCatalogInput<'_, '_>,
    tree: &RecordTree<'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    plan: &TemplatePlan<'fields, 'scope, 'read, 'source>,
    meanings: FlowMeanings<'_, 'fields, 'scope, 'read, 'source>,
    projected: Result<BrokerEmission<'source>, Box<BrokerProjectionFailure<'source>>>,
) -> Result<CanonicalBroker<'source>, CatalogError> {
    let members = meanings.members();
    let (flows, mission_flows) =
        crate::template_flow_configuration_read::read(members, plan, FAMILY)?;
    let (properties, mission_properties) =
        crate::template_property_configuration_read::read(members, &plan.objectives, FAMILY)?;
    let definition = BrokerDefinition::emit(
        input,
        tree,
        references,
        meanings,
        (mission_flows, mission_properties),
    )?;
    Ok(CanonicalBroker {
        definition,
        flows,
        properties,
        projection: projected,
    })
}

fn root_spawns<'fields, 'scope, 'read, 'source>(
    spawns: &mut TemplateSpawns<'fields, 'scope, 'read, 'source>,
    references: &RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
    tags: &TemplateTagCache<'_, 'fields, 'scope, 'read, 'source>,
    sources: &ContextSources<'_, '_, 'source>,
    plan: &ContextPlan<'_, 'fields, 'scope, 'read, 'source>,
) -> Result<(), ReadError> {
    for declaration in &plan.spawns.root {
        spawns.required(
            references,
            tags,
            sources.tree,
            declaration.value,
            sources.capture,
        )?;
    }
    Ok(())
}

fn failed<'source>(
    admission: &RecordObservation<'_, 'source>,
    error: ReadError,
) -> Result<Box<BrokerProjectionFailure<'source>>, CatalogError> {
    match error.downcast::<CatalogError>() {
        Ok(error) => Err(*error),
        Err(error) => Ok(Box::new(BrokerProjectionFailure::read(
            admission,
            error.as_ref(),
        )?)),
    }
}
