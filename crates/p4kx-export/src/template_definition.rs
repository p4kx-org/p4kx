// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::PhysicalSourceRef;
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::template_configuration::{MissionFlowRef, MissionPropertyRef};
use crate::template_number_capture::TemplateNumbers;
use crate::template_source::TemplateSource;

/// One physical Template owner with a single complete selected original-value inventory.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateDefinition<'source> {
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
