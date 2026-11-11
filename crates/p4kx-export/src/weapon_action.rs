// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::authored_nodes::AuthoredNodes;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

#[path = "weapon_action_beam.rs"]
mod beam;
#[path = "weapon_action_charge.rs"]
mod charge;
#[path = "weapon_action_condition.rs"]
mod condition;
#[path = "weapon_launcher.rs"]
mod launcher;
#[path = "weapon_action_sequence.rs"]
mod sequence;

use beam::BeamAction;
use charge::ChargedAction;
use condition::{ConditionalAction, ToggleAction};
use launcher::WeaponLauncher;
use sequence::{ParallelAction, SequenceAction};

/// An action node and its stable original occurrence within the physical Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct WeaponAction<'source> {
    source_type: SourcedValue<&'source str>,
    source_value_ordinal: SourcedValue<DecimalU64>,
    kind: WeaponActionKind<'source>,
}

impl<'source> WeaponAction<'source> {
    pub(super) fn read<'reader, 'scope, 'read>(
        source: Observed<'reader, 'scope, 'read, 'source>,
        pending: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source_type: source.source_type_value()?,
            source_value_ordinal: source.owner_ordinal()?,
            kind: WeaponActionKind::read(source, pending, notes)?,
        })
    }
}

/// Typed action inputs and the original bodies of other admitted action declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) enum WeaponActionKind<'source> {
    Single(Box<ShotInputs<'source>>),
    Rapid(Box<RapidAction<'source>>),
    Burst(Box<BurstAction<'source>>),
    Beam(Box<BeamAction<'source>>),
    Sequence(Box<SequenceAction<'source>>),
    Parallel(Box<ParallelAction<'source>>),
    Conditional(Box<ConditionalAction<'source>>),
    DynamicConditional(Box<ConditionalAction<'source>>),
    Toggle(Box<ToggleAction<'source>>),
    Charged(Box<ChargedAction<'source>>),
    Authored(SourceArenaOutput<'source>),
}

impl<'source> WeaponActionKind<'source> {
    fn read<'reader, 'scope, 'read>(
        source: Observed<'reader, 'scope, 'read, 'source>,
        pending: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "SWeaponActionFireSingleParams" => {
                Self::Single(Box::new(ShotInputs::read(source, notes, [])?))
            }
            "SWeaponActionFireRapidParams" => {
                Self::Rapid(Box::new(RapidAction::read(source, notes)?))
            }
            "SWeaponActionFireBurstParams" => {
                Self::Burst(Box::new(BurstAction::read(source, notes)?))
            }
            "SWeaponActionFireBeamParams" => Self::Beam(Box::new(BeamAction::read(source, notes)?)),
            "SWeaponActionSequenceParams" => {
                Self::Sequence(Box::new(SequenceAction::read(source, pending, notes)?))
            }
            "SWeaponActionParallelParams" => {
                Self::Parallel(Box::new(ParallelAction::read(source, pending, notes)?))
            }
            "SWeaponActionConditionParams" => {
                Self::Conditional(Box::new(ConditionalAction::read(source, pending, notes)?))
            }
            "SWeaponActionDynamicConditionParams" => {
                Self::DynamicConditional(Box::new(ConditionalAction::read(source, pending, notes)?))
            }
            "SWeaponActionToggleParams" => {
                Self::Toggle(Box::new(ToggleAction::read(source, pending, notes)?))
            }
            "SWeaponActionFireChargedParams" => {
                Self::Charged(Box::new(ChargedAction::read(source, pending, notes)?))
            }
            _ => Self::Authored(source.authored_members()?),
        })
    }
}

/// Stored per-shot values and the actual owned launcher.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShotInputs<'source> {
    launch_params: SourcedValue<WeaponLauncher<'source>>,
    fire_rate: SourcedValue<i32>,
    heat_per_shot: SourcedValue<ResourceFloat32>,
    wear_per_shot: SourcedValue<ResourceFloat32>,
    authored: SourceArenaOutput<'source>,
}

impl<'source> ShotInputs<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
        projected: impl IntoIterator<Item = (&'static str, SourceValueState)>,
    ) -> Result<Self, CatalogError> {
        let launch_params = WeaponLauncher::read_field(source, notes)?;
        let fire_rate = source.int32("fireRate", notes)?;
        let heat_per_shot = source.single("heatPerShot", notes)?;
        let wear_per_shot = source.single("wearPerShot", notes)?;
        let authored = source.authored_members_except(
            [
                ("launchParams", launch_params.state()),
                ("fireRate", fire_rate.state()),
                ("heatPerShot", heat_per_shot.state()),
                ("wearPerShot", wear_per_shot.state()),
            ]
            .into_iter()
            .chain(projected),
        )?;
        Ok(Self {
            launch_params,
            fire_rate,
            heat_per_shot,
            wear_per_shot,
            authored,
        })
    }
}

/// The original per-shot and spin inputs of a rapid-fire action.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RapidAction<'source> {
    #[serde(flatten)]
    shot: ShotInputs<'source>,
    fire_during_spin_up: SourcedValue<bool>,
    spin_up_time: SourcedValue<ResourceFloat32>,
    spin_down_time: SourcedValue<ResourceFloat32>,
    spin_param: SourcedValue<&'source str>,
    audio_loop_time_between_first_shots: SourcedValue<ResourceFloat32>,
}

impl<'source> RapidAction<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let fire_during_spin_up = source.boolean("fireDuringSpinUp", notes)?;
        let spin_up_time = source.single("spinUpTime", notes)?;
        let spin_down_time = source.single("spinDownTime", notes)?;
        let spin_param = source.borrowed_string("spinParam", notes)?;
        let audio_loop_time_between_first_shots =
            source.single("audioLoopTimeBetweenFirstShots", notes)?;
        let shot = ShotInputs::read(
            source,
            notes,
            [
                ("fireDuringSpinUp", fire_during_spin_up.state()),
                ("spinUpTime", spin_up_time.state()),
                ("spinDownTime", spin_down_time.state()),
                ("spinParam", spin_param.state()),
                (
                    "audioLoopTimeBetweenFirstShots",
                    audio_loop_time_between_first_shots.state(),
                ),
            ],
        )?;
        Ok(Self {
            shot,
            fire_during_spin_up,
            spin_up_time,
            spin_down_time,
            spin_param,
            audio_loop_time_between_first_shots,
        })
    }
}

/// The original per-shot, count and cooldown inputs of a burst action.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BurstAction<'source> {
    #[serde(flatten)]
    shot: ShotInputs<'source>,
    shot_count: SourcedValue<u8>,
    cooldown_time: SourcedValue<ResourceFloat32>,
}

impl<'source> BurstAction<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let shot_count = source.uint8("shotCount", notes)?;
        let cooldown_time = source.single("cooldownTime", notes)?;
        let shot = ShotInputs::read(
            source,
            notes,
            [
                ("shotCount", shot_count.state()),
                ("cooldownTime", cooldown_time.state()),
            ],
        )?;
        Ok(Self {
            shot,
            shot_count,
            cooldown_time,
        })
    }
}
