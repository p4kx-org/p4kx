// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, PhysicalSourceRef, Record, RecordTree};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_template_modules::template_prepared_owner::PreparedOwner;
use crate::mission_flow_owner::FlowOwnerKind;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::template_configuration::{MissionFlowRef, MissionPropertyRef};
use crate::template_field_capture::TemplateFieldCapture;
use crate::template_source::TemplateSource;

/// One physical Template owner with a single complete selected original-value inventory.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct TemplateDefinition<'source> {
    input_ref: PhysicalSourceRef,
    #[serde(flatten)]
    header: CanonicalRecordHeader<'source>,
    source: TemplateSource<'source>,
    mission_flows: Vec<MissionFlowRef>,
    mission_properties: Vec<MissionPropertyRef>,
}

impl<'source> TemplateDefinition<'source> {
    /// Borrows the original DataForge source identity.
    #[must_use]
    pub const fn input_ref(&self) -> &PhysicalSourceRef {
        &self.input_ref
    }

    /// Borrows the physical Template header with its field origins.
    #[must_use]
    pub const fn header(&self) -> &CanonicalRecordHeader<'source> {
        &self.header
    }

    /// Borrows the original source arena shared by the declaration links.
    #[must_use]
    pub const fn source(&self) -> &TemplateSource<'source> {
        &self.source
    }

    /// Borrows the links to this owner's canonical flow declarations.
    #[must_use]
    pub fn mission_flows(&self) -> &[MissionFlowRef] {
        &self.mission_flows
    }

    /// Borrows the links to this owner's canonical property declarations.
    #[must_use]
    pub fn mission_properties(&self) -> &[MissionPropertyRef] {
        &self.mission_properties
    }

    pub(crate) fn emit(
        input: &OriginalCatalogInput<'_, 'source>,
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
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<TemplateDefinition<'source>>, CatalogError> {
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
    let (_, flows) = crate::template_flow_configuration_read::read(&members, &plan, owner)?;
    let (_, properties) =
        crate::template_property_configuration_read::read(&members, &plan.objectives, owner)?;
    let (source, failure) =
        crate::template_meanings::read(&members, &plan.flow, references.get_mut(), None, &capture)?;
    let complete = failure.is_none() && source.values_are_readable();
    let row = TemplateDefinition::emit(input, &fields, tree, source, (flows, properties))?;
    Ok(ScopedSourceRead::new(row, Vec::new(), complete))
}
