// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Record, RecordTree};

use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_rows::mission_definitions::DefinitionSource;
use crate::catalog_template_modules::{
    mission_owner_definition::MissionOwnerDefinition, template_prepared_owner::PreparedOwner,
};
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::export_ledger::ExportLedger;
use crate::mission_flow_owner::FlowOwnerKind;
use crate::record_observation::RecordObservation;
use crate::spawn_pools::SpawnPools;
use crate::template_configuration::{MissionFlowConfiguration, MissionPropertyConfiguration};
use crate::template_context_read::{ContextPlan, ContextSources};
use crate::template_definition::TemplateDefinition;
use crate::template_field_capture::TemplateFieldCapture;
use crate::template_issues::Unknowns;
use crate::template_number_capture::TemplateNumbers;
use crate::template_projection::TemplateProjection;
use crate::{CatalogError, CatalogReadFailure};

/// One complete D owner and its separate required C interpretations.
pub(crate) struct CanonicalTemplate<'source> {
    pub(crate) definition: TemplateDefinition<'source>,
    pub(crate) definition_source: DefinitionSource<'source>,
    pub(crate) projection: TemplateProjection<'source>,
    pub(crate) flows: Vec<MissionFlowConfiguration>,
    pub(crate) properties: Vec<MissionPropertyConfiguration>,
    pub(crate) pools: SpawnPools<'source>,
    pub(crate) ledger: ExportLedger,
    pub(crate) failures: Vec<CatalogReadFailure<'source>>,
}

pub(crate) fn read<'source>(
    projection: &CatalogProjection<'_, 'source>,
    tags: &CapturedTags<'_, 'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    cancel: Option<&CancelToken>,
    unknowns: &Unknowns,
) -> Result<CanonicalTemplate<'source>, CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(record, tree)?;
    let fields = OriginalFlowFields::read(&scope, cancel)?;
    let admission = RecordObservation::capture(input, record)?;
    let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
    let owner = FlowOwnerKind::ContractTemplate;
    let PreparedOwner {
        plan,
        members,
        mut references,
    } = PreparedOwner::read(&fields, &capture, owner)?;
    let (flows, flow_links) =
        crate::template_flow_configuration_read::read(&members, &plan, owner)?;
    let (properties, property_links) =
        crate::template_property_configuration_read::read(&members, &plan.objectives, owner)?;
    let mut context = crate::template_context_read::read(
        &members,
        &mut references,
        &ContextSources {
            tags,
            tree,
            projection,
            unknowns,
            capture: &capture,
        },
        &ContextPlan::from_plan(&plan),
    )?;
    let (source, flow_failure) = crate::template_meanings::read(
        &members,
        &plan.flow,
        references.get_mut(),
        Some(unknowns),
        &capture,
    )?;
    context.errors.retain(
        input,
        record,
        flow_failure,
        &mut context.ledger,
        &mut context.failures,
    )?;
    let definition_source = DefinitionSource::new(tree, TemplateNumbers::read(&members, input)?);
    let definition = TemplateDefinition::from(MissionOwnerDefinition::read(
        input,
        &fields,
        tree,
        source,
        (flow_links, property_links),
    )?);
    Ok(CanonicalTemplate {
        definition,
        definition_source,
        projection: context.projection,
        flows,
        properties,
        pools: context.pools,
        ledger: context.ledger,
        failures: context.failures,
    })
}
