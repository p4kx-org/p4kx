// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, PhysicalSourceRef, Record, RecordTree};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_template_modules::template_prepared_owner::PreparedOwner;
use crate::mission_flow_owner::FlowOwnerKind;
use crate::record_observation::RecordObservation;
use crate::template_configuration::{MissionFlowRef, MissionPropertyRef};
use crate::template_field_capture::TemplateFieldCapture;
use crate::template_source::TemplateSource;

pub(crate) struct MissionOwnerDefinition<'source> {
    pub(crate) input_ref: PhysicalSourceRef,
    pub(crate) header: CanonicalRecordHeader<'source>,
    pub(crate) source: TemplateSource<'source>,
    pub(crate) mission_flows: Vec<MissionFlowRef>,
    pub(crate) mission_properties: Vec<MissionPropertyRef>,
}

impl<'source> MissionOwnerDefinition<'source> {
    pub(crate) fn read(
        input: &OriginalCatalogInput<'_, '_>,
        fields: &OriginalFlowFields<'_, '_, 'source>,
        tree: &RecordTree<'source>,
        source: TemplateSource<'source>,
        (mission_flows, mission_properties): (Vec<MissionFlowRef>, Vec<MissionPropertyRef>),
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            input_ref: fields.input_ref().clone(),
            header: CanonicalRecordHeader::read(tree, input.origins())?,
            source,
            mission_flows,
            mission_properties,
        })
    }
}

pub(crate) fn read_selected<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    owner: FlowOwnerKind,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<MissionOwnerDefinition<'source>>, CatalogError> {
    let scope = input.record_scope(record, tree)?;
    let fields = OriginalFlowFields::read(&scope, cancel)?;
    let admission = RecordObservation::capture(input, record)?;
    let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
    let PreparedOwner {
        plan,
        members,
        mut references,
    } = PreparedOwner::read(&fields, &capture, owner)?;
    let (_, flows) = crate::template_flow_configuration_read::read(&members, &plan, owner)?;
    let (_, properties) =
        crate::template_property_configuration_read::read(&members, &plan.objectives, owner)?;
    let (source, failure) =
        crate::template_meanings::read(&members, &plan.flow, references.get_mut(), None, &capture)?;
    let complete = failure.is_none() && source.values_are_readable();
    let row = MissionOwnerDefinition::read(input, &fields, tree, source, (flows, properties))?;
    Ok(ScopedSourceRead::new(row, Vec::new(), complete))
}
