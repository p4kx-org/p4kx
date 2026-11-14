// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, Record, RecordTree};

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::mission_flow_owner::FlowOwnerKind;
use crate::record_observation::RecordObservation;
use crate::spawn_pools::SpawnPools;
use crate::template_configuration::{MissionFlowConfiguration, MissionPropertyConfiguration};
use crate::template_context_read::{ContextPlan, ContextSources};
use crate::template_definition::TemplateDefinition;
use crate::template_field_capture::TemplateFieldCapture;
use crate::template_issues::Unknowns;
use crate::template_plan::TemplatePlan;
use crate::template_projection::TemplateProjection;
use crate::template_reference_index::TemplateReferences;
use crate::template_source_members::TemplateMembers;
use crate::{CatalogError, CatalogReadFailure};

/// One complete D owner and its separate required C interpretations.
pub(crate) struct CanonicalTemplate<'source> {
    pub(crate) definition: TemplateDefinition<'source>,
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
    let mut references = RefCell::new(TemplateReferences::new(&fields));
    let owner = FlowOwnerKind::ContractTemplate;
    let plan = TemplatePlan::read(&fields, references.get_mut(), owner)?;
    let members = TemplateMembers::new(&fields, plan.roots(), owner)?
        .with_warning_capture(capture.example_body());
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
    let definition = TemplateDefinition {
        physical_guid: CatalogGuid(tree.guid()),
        physical_path: tree.record_path(),
        numbers: crate::template_number_capture::TemplateNumbers::read(&members, input)?,
        input_ref: fields.input_ref().clone(),
        header: CanonicalRecordHeader::read(tree, projection.origins())?,
        source,
        mission_flows: flow_links,
        mission_properties: property_links,
    };
    Ok(CanonicalTemplate {
        definition,
        projection: context.projection,
        flows,
        properties,
        pools: context.pools,
        ledger: context.ledger,
        failures: context.failures,
    })
}
