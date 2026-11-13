// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::Transform;
use p4kx_extraction::{SourceUse, SourceUses, SourceValue, SourceValueState, SourcedValue};

use super::super::authored_fields::RoomTransformUnavailable;
use super::super::room_associations::RoomAssociation;
use super::walk::RootRoomTransform;
use crate::catalog_read::original::OriginalDatum;

type Matrix = [[f32; 4]; 3];

#[derive(Clone, Copy)]
pub(super) enum RoomFrame<'source> {
    DirectItem,
    Vehicle(&'source RoomAssociation<'source>),
    Attached,
}

impl<'source> RoomFrame<'source> {
    pub(super) const fn read(
        attached: bool,
        room: Option<&'source RoomAssociation<'source>>,
    ) -> Self {
        match (attached, room) {
            (true, _) => Self::Attached,
            (false, Some(room)) => Self::Vehicle(room),
            (false, None) => Self::DirectItem,
        }
    }

    pub(super) fn for_entity(
        self,
        sources: &SourceUses,
        nested: Option<&RootRoomTransform>,
        local: &OriginalDatum<Matrix>,
        route: &[SourceUse],
        route_complete: bool,
    ) -> SourcedValue<Matrix> {
        let (entity_inputs, local_complete) = local.contributions();
        let mut inputs = route.to_vec();
        let room = self.transform(&mut inputs);
        if room.is_ok() {
            inputs.extend_from_slice(entity_inputs);
            if let Some(nested) = nested {
                inputs.extend_from_slice(&nested.uses);
            }
        }
        let (value, complete) = match room {
            Ok(room) => (compose(room, nested, local), local_complete),
            Err(state) => (state.value(), state.is_complete()),
        };
        sources.value(value, &inputs, route_complete && complete)
    }

    fn transform(self, inputs: &mut Vec<SourceUse>) -> Result<Transform, RoomTransformUnavailable> {
        let room = match self {
            Self::DirectItem => return Ok(Transform::IDENTITY),
            Self::Attached => return Err(RoomTransformUnavailable::Unsupported),
            Self::Vehicle(room) => room,
        };
        inputs.push(room.bone_input.clone());
        match room.bone_name.state() {
            SourceValueState::Absent => {}
            SourceValueState::Present
                if room
                    .bone_name
                    .value()
                    .is_some_and(|name| name.trim().is_empty()) => {}
            SourceValueState::Present | SourceValueState::Unsupported => {
                return Err(RoomTransformUnavailable::Unsupported);
            }
            SourceValueState::Unreadable => return Err(RoomTransformUnavailable::Unreadable),
            SourceValueState::Null => return Err(RoomTransformUnavailable::Null),
            _ => return Err(RoomTransformUnavailable::Invalid),
        }
        inputs.extend_from_slice(&room.room_offset.inputs);
        room.room_offset.transform
    }
}

fn compose(
    room: Transform,
    nested: Option<&RootRoomTransform>,
    local: &OriginalDatum<Matrix>,
) -> SourceValue<Matrix> {
    let Some(room) = nested.map_or(Some(room), |nested| {
        nested.value.and_then(|nested| room.then(&nested).finite())
    }) else {
        return SourceValue::Invalid;
    };
    match local.value() {
        SourceValue::Present(rows) => room
            .then(&Transform { rows: *rows })
            .finite()
            .map_or(SourceValue::Invalid, |transform| {
                SourceValue::Present(transform.rows)
            }),
        SourceValue::Absent => SourceValue::Absent,
        SourceValue::Null => SourceValue::Null,
        SourceValue::Unreadable => SourceValue::Unreadable,
        SourceValue::Unsupported => SourceValue::Unsupported,
        _ => SourceValue::Invalid,
    }
}
