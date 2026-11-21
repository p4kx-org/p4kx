// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{PhysicalSourceRef, RecordTree};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::template_configuration::{MissionFlowRef, MissionPropertyRef};
use crate::template_meanings::FlowMeanings;
use crate::template_number_capture::TemplateNumbers;
use crate::template_reference_index::TemplateReferences;
use crate::template_source::TemplateSource;

/// One physical Broker owner and its single complete original-value inventory.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BrokerDefinition<'source> {
    #[serde(skip)]
    pub(crate) physical_guid: CatalogGuid,
    #[serde(skip)]
    pub(crate) physical_path: &'source str,
    #[serde(skip)]
    pub(crate) numbers: TemplateNumbers,
    pub(crate) input_ref: PhysicalSourceRef,
    #[serde(flatten)]
    pub(crate) header: CanonicalRecordHeader<'source>,
    pub(crate) source: TemplateSource<'source>,
    pub(crate) mission_flows: Vec<MissionFlowRef>,
    pub(crate) mission_properties: Vec<MissionPropertyRef>,
}

impl<'source> BrokerDefinition<'source> {
    pub(crate) fn emit<'fields, 'scope, 'read>(
        input: &OriginalCatalogInput<'_, '_>,
        tree: &RecordTree<'source>,
        references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
        meanings: FlowMeanings<'_, 'fields, 'scope, 'read, 'source>,
        (mission_flows, mission_properties): (Vec<MissionFlowRef>, Vec<MissionPropertyRef>),
    ) -> Result<Self, CatalogError> {
        let members = meanings.members();
        members.fields().check_tree(tree)?;
        let source = meanings.emit(references)?;
        Ok(Self {
            physical_guid: CatalogGuid(tree.guid()),
            physical_path: tree.record_path(),
            numbers: TemplateNumbers::broker(members, input)?,
            input_ref: members.fields().input_ref().clone(),
            header: CanonicalRecordHeader::read(tree, input.origins())?,
            source,
            mission_flows,
            mission_properties,
        })
    }
}
