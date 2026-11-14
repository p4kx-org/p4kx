// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Original actor-state predicate labels of one authored setup.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ActorStateFilter<'source> {
    #[serde(rename = "filterName")]
    name: SourcedValue<&'source str>,
    #[serde(rename = "filterByState")]
    state: SourcedValue<&'source str>,
    #[serde(rename = "filterByMotionSpeed")]
    motion_speed: SourcedValue<&'source str>,
    #[serde(rename = "filterByPoseState")]
    pose_state: SourcedValue<&'source str>,
    #[serde(rename = "filterByStanceState")]
    stance_state: SourcedValue<&'source str>,
    #[serde(rename = "filterByAimStanceState")]
    aim_stance_state: SourcedValue<&'source str>,
    #[serde(rename = "filterByLeanState")]
    lean_state: SourcedValue<&'source str>,
    #[serde(rename = "filterByHeldItemType")]
    held_item_type: SourcedValue<&'source str>,
    #[serde(rename = "filterByEquippedArmorClass")]
    equipped_armor_class: SourcedValue<&'source str>,
    #[serde(rename = "filterBySkeleton")]
    skeleton: SourcedValue<&'source str>,
    #[serde(rename = "filterByCharacterType")]
    character_type: SourcedValue<&'source str>,
    #[serde(rename = "filterByRestrainedState")]
    restrained_state: SourcedValue<&'source str>,
    #[serde(rename = "filterByPlayerCamera")]
    player_camera: SourcedValue<&'source str>,
    #[serde(rename = "filterByAimingRestriction")]
    aiming_restriction: SourcedValue<&'source str>,
}

impl<'source> ActorStateFilter<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            name: fields.borrowed_string("filterName", notes)?,
            state: fields.borrowed_declared_enum("filterByState", notes)?,
            motion_speed: fields.borrowed_declared_enum("filterByMotionSpeed", notes)?,
            pose_state: fields.borrowed_declared_enum("filterByPoseState", notes)?,
            stance_state: fields.borrowed_declared_enum("filterByStanceState", notes)?,
            aim_stance_state: fields.borrowed_declared_enum("filterByAimStanceState", notes)?,
            lean_state: fields.borrowed_declared_enum("filterByLeanState", notes)?,
            held_item_type: fields.borrowed_declared_enum("filterByHeldItemType", notes)?,
            equipped_armor_class: fields
                .borrowed_declared_enum("filterByEquippedArmorClass", notes)?,
            skeleton: fields.borrowed_declared_enum("filterBySkeleton", notes)?,
            character_type: fields.borrowed_declared_enum("filterByCharacterType", notes)?,
            restrained_state: fields.borrowed_declared_enum("filterByRestrainedState", notes)?,
            player_camera: fields.borrowed_declared_enum("filterByPlayerCamera", notes)?,
            aiming_restriction: fields
                .borrowed_declared_enum("filterByAimingRestriction", notes)?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 14] {
        [
            ("filterName", self.name.state()),
            ("filterByState", self.state.state()),
            ("filterByMotionSpeed", self.motion_speed.state()),
            ("filterByPoseState", self.pose_state.state()),
            ("filterByStanceState", self.stance_state.state()),
            ("filterByAimStanceState", self.aim_stance_state.state()),
            ("filterByLeanState", self.lean_state.state()),
            ("filterByHeldItemType", self.held_item_type.state()),
            (
                "filterByEquippedArmorClass",
                self.equipped_armor_class.state(),
            ),
            ("filterBySkeleton", self.skeleton.state()),
            ("filterByCharacterType", self.character_type.state()),
            ("filterByRestrainedState", self.restrained_state.state()),
            ("filterByPlayerCamera", self.player_camera.state()),
            ("filterByAimingRestriction", self.aiming_restriction.state()),
        ]
    }
}
