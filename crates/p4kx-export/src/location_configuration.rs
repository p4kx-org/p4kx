// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};
use crate::template_field_capture::TemplateFieldCapture;

const SOURCE_TYPE: &str = "MissionLocationData";
pub(crate) const SECURITY_MANIFEST_TYPE: &str = "SecurityNetworkManifest";

/// Location-owned limits and its authored default security definition link.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionLocationConfiguration<'source> {
    mission_limits: SourcedValue<Vec<SourcedValue<LocationMissionLimit<'source>>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    default_security_network_manifest: SourcedValue<Box<SourceRecordReference<'source>>>,
}

impl<'source> MissionLocationConfiguration<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            mission_limits: fields.class_array_to(
                "missionLimits",
                "LocationMissionLimit",
                notes,
                LocationMissionLimit::read,
            )?,
            default_security_network_manifest: fields.record_link_to(
                "defaultSecurityNetworkManifest",
                SECURITY_MANIFEST_TYPE,
                notes,
            )?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct LocationMissionLimit<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    tag: SourcedValue<Box<SourceRecordReference<'source>>>,
    auto_consume_if_player_at_location: SourcedValue<bool>,
    max_mission_instances: SourcedValue<i32>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> LocationMissionLimit<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let tag = fields.record_link_to("tag", "Tag", notes)?;
        let auto_consume_if_player_at_location =
            fields.boolean("autoConsumeIfPlayerAtLocation", notes)?;
        let max_mission_instances = fields.int32("maxMissionInstances", notes)?;
        let authored_fields = fields.authored_members_except([
            ("tag", tag.state()),
            (
                "autoConsumeIfPlayerAtLocation",
                auto_consume_if_player_at_location.state(),
            ),
            ("maxMissionInstances", max_mission_instances.state()),
        ])?;
        Ok(Self {
            tag,
            auto_consume_if_player_at_location,
            max_mission_instances,
            authored_fields,
        })
    }
}

pub(crate) fn read<'source>(
    originals: &OriginalFlowFields<'_, '_, 'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    capture: &TemplateFieldCapture<'_, '_, '_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<MissionLocationConfiguration<'source>>, CatalogError> {
    let numbers = RefCell::default();
    let fields = Observed::definition(originals, input, capture, &numbers);
    fields.declared_class_to(
        "locationData",
        SOURCE_TYPE,
        notes,
        MissionLocationConfiguration::read,
    )
}
