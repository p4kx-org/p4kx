// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_seat_ejection::{self, SeatEjection};
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(super) const COMPONENT: &str = "SCItemSeatParams";
const SEAT_TYPE: &str = "SeatTypes";
const OPERATOR_MODES: &str = "OperatorModeAvailabilityParams";
const ACTOR_ATTACHMENT: &str = "SCSeatActorAttachmentParams";

/// The stored seat controls, ranges and attachment declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct Seat<'source> {
    #[serde(rename = "seatType")]
    type_label: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    available_operator_modes: SourcedValue<Box<SourceRecordReference<'source>>>,
    control_template: SourcedValue<&'source str>,
    transition_template: SourcedValue<&'source str>,
    min_yaw: SourcedValue<ResourceFloat32>,
    max_yaw: SourcedValue<ResourceFloat32>,
    min_pitch: SourcedValue<ResourceFloat32>,
    max_pitch: SourcedValue<ResourceFloat32>,
    set_yaw_pitch_limits: SourcedValue<bool>,
    actor_attachment: SourcedValue<SeatActorAttachment<'source>>,
    ejection: SourcedValue<SeatEjection<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> Seat<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let type_label = source.borrowed_enum_to("seatType", SEAT_TYPE, notes)?;
        let available_operator_modes =
            source.record_link_to("availableOperatorModes", OPERATOR_MODES, notes)?;
        let control_template = source.borrowed_string("controlTemplate", notes)?;
        let transition_template = source.borrowed_string("transitionTemplate", notes)?;
        let min_yaw = source.single("minYaw", notes)?;
        let max_yaw = source.single("maxYaw", notes)?;
        let min_pitch = source.single("minPitch", notes)?;
        let max_pitch = source.single("maxPitch", notes)?;
        let set_yaw_pitch_limits = source.boolean("setYawPitchLimits", notes)?;
        let actor_attachment = source.declared_class_to(
            "actorAttachment",
            ACTOR_ATTACHMENT,
            notes,
            SeatActorAttachment::read,
        )?;
        let ejection = source.strong_pointer_to(
            "ejection",
            catalog_seat_ejection::SOURCE_TYPE,
            &[catalog_seat_ejection::SOURCE_TYPE],
            notes,
            SeatEjection::read,
        )?;
        let additional_fields = source.authored_members_except([
            ("seatType", type_label.state()),
            ("availableOperatorModes", available_operator_modes.state()),
            ("controlTemplate", control_template.state()),
            ("transitionTemplate", transition_template.state()),
            ("minYaw", min_yaw.state()),
            ("maxYaw", max_yaw.state()),
            ("minPitch", min_pitch.state()),
            ("maxPitch", max_pitch.state()),
            ("setYawPitchLimits", set_yaw_pitch_limits.state()),
            ("actorAttachment", actor_attachment.state()),
            ("ejection", ejection.state()),
        ])?;
        Ok(Self {
            type_label,
            available_operator_modes,
            control_template,
            transition_template,
            min_yaw,
            max_yaw,
            min_pitch,
            max_pitch,
            set_yaw_pitch_limits,
            actor_attachment,
            ejection,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct SeatActorAttachment<'source> {
    source_type: SourcedValue<&'source str>,
    r#type: SourcedValue<&'source str>,
    bone_name: SourcedValue<&'source str>,
    stance: SourcedValue<&'source str>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> SeatActorAttachment<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let r#type = source.borrowed_enum_to("type", "SCSeatActorAttachmentType", notes)?;
        let bone_name = source.borrowed_string("boneName", notes)?;
        let stance = source.borrowed_enum_to("stance", "ActorStateFilterByStanceState", notes)?;
        let additional_fields = source.authored_members_except([
            ("type", r#type.state()),
            ("boneName", bone_name.state()),
            ("stance", stance.state()),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            r#type,
            bone_name,
            stance,
            additional_fields,
        })
    }
}
