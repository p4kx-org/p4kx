// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::ops::Range;

use p4kx_character::RigLogic;
use p4kx_model::{
    AnimationController, AnimationSource, PositionedAnimation, RotationIdentityEvidence,
};
use serde_json::{Value, json};

const CHANNEL_DESCRIPTOR_BYTES: u64 = 12;
const KEY_COUNT_BYTES: u64 = 2;

#[derive(Debug)]
enum ControlState {
    DrivenScalar,
    ConstantInput,
    ModeledNeutral(Option<RotationIdentityEvidence>),
}

impl ControlState {
    fn read(
        original: &AnimationController<'_>,
        control: usize,
        scalar_keys: u16,
    ) -> Result<Self, String> {
        if scalar_keys != 0 {
            return Ok(if control == 0 {
                Self::ConstantInput
            } else {
                Self::DrivenScalar
            });
        }
        let rotation = original
            .rotation_identity()
            .map_err(|error| error.to_string())?;
        if rotation
            .as_ref()
            .is_some_and(|evidence| !evidence.is_canonical())
        {
            return Err(
                "standalone scalar-less control has active or noncanonical rotation payload"
                    .to_owned(),
            );
        }
        Ok(Self::ModeledNeutral(rotation))
    }

    const fn code(&self) -> &'static str {
        match self {
            Self::DrivenScalar => "drivenScalar",
            Self::ConstantInput => "constantInput",
            Self::ModeledNeutral(_) => "modeledNeutral",
        }
    }
}

#[derive(Debug)]
pub(super) struct ControlEvidence {
    ordinal: usize,
    decoded_track: Option<usize>,
    controller: u32,
    control: usize,
    state: ControlState,
    id: Range<u64>,
    descriptor: Range<u64>,
    rotation_keys: u16,
    scalar_keys: u16,
}

impl ControlEvidence {
    pub(super) const fn projected_track(&self) -> Option<usize> {
        match &self.state {
            ControlState::ModeledNeutral(_) => self.decoded_track,
            ControlState::DrivenScalar | ControlState::ConstantInput => None,
        }
    }

    pub(super) fn record(&self) -> Value {
        let modeled = match &self.state {
            ControlState::DrivenScalar => None,
            ControlState::ConstantInput | ControlState::ModeledNeutral(_) => {
                Some(if self.control == 0 { 1.0 } else { 0.0 })
            }
        };
        let rotation = match &self.state {
            ControlState::ModeledNeutral(rotation) => rotation.as_ref(),
            ControlState::DrivenScalar | ControlState::ConstantInput => None,
        };
        let range = |span: Range<u64>| [span.start, span.end];
        json!({
            "record": "standaloneFaceControl",
            "controllerOrdinal": self.ordinal,
            "decodedTrack": self.decoded_track,
            "controllerId": self.controller,
            "controlSlot": self.control,
            "state": self.state.code(),
            "modeledValue": modeled,
            "authoredScalarKeyCount": self.scalar_keys,
            "authoredRotationKeyCount": self.rotation_keys,
            "sourceBytes": {
                "controllerId": range(self.id.clone()),
                "controllerDescriptor": range(self.descriptor.clone()),
                "scalarKeyCount": [self.descriptor.start + CHANNEL_DESCRIPTOR_BYTES,
                    self.descriptor.start + CHANNEL_DESCRIPTOR_BYTES + KEY_COUNT_BYTES],
                "rotationFrames": rotation.map(|evidence| range(evidence.frame_range())),
                "rotationValues": rotation.map(|evidence| range(evidence.value_range())),
            },
            "canonicalIdentityRotation": rotation.map(RotationIdentityEvidence::is_canonical),
        })
    }
}

pub(super) fn controls(
    source: &AnimationSource<'_>,
    positioned: &PositionedAnimation,
    logic: &RigLogic,
) -> Result<Vec<ControlEvidence>, String> {
    let mut decoded = positioned.animation().tracks.iter().enumerate();
    let mut targets = HashSet::new();
    let mut out = Vec::new();
    let mut usable = false;
    for original in source.controllers().map_err(|error| error.to_string())? {
        let control = logic
            .control_by_hash(original.controller_id())
            .ok_or("standalone comparison has an unknown rig control")?;
        if !targets.insert(control) {
            return Err(
                "standalone comparison has repeated original records for one rig control"
                    .to_owned(),
            );
        }
        let rotation_keys = original.rotation().key_count;
        let scalar_keys = original.translation().key_count;
        let decoded_track = if rotation_keys != 0 || scalar_keys != 0 {
            let (index, track) = decoded
                .next()
                .ok_or("source controller has no paired decoded track")?;
            if positioned.controller_ordinal(index) != Some(original.ordinal())
                || track.controller_id != original.controller_id()
                || track.translations.is_some() != (scalar_keys != 0)
                || track.rotations.is_some() != (rotation_keys != 0)
            {
                return Err("source controller differs from its paired canonical decode".to_owned());
            }
            Some(index)
        } else {
            None
        };
        let state = ControlState::read(&original, control, scalar_keys)?;
        usable |= matches!(state, ControlState::DrivenScalar);
        out.push(ControlEvidence {
            ordinal: original.ordinal(),
            decoded_track,
            controller: original.controller_id(),
            control,
            state,
            id: original.id_range(),
            descriptor: original.table_range(),
            rotation_keys,
            scalar_keys,
        });
    }
    if decoded.next().is_some() {
        return Err("decoded tracks remain outside the original controller table".to_owned());
    }
    if !usable {
        return Err("standalone comparison needs a keyed nonconstant facial control".to_owned());
    }
    Ok(out)
}
