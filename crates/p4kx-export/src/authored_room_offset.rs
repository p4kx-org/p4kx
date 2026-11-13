// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::Transform;
use p4kx_extraction::{DataType, SourceUse, SourceUseRole, SourceValue, Value};

use super::{Fact, Observed};
use crate::CatalogError;
use crate::catalog_read::original::OriginalValuePart;

#[derive(Debug, Clone, Copy)]
pub(in crate::catalog_v3) enum RoomTransformUnavailable {
    Absent,
    Null,
    Unreadable,
    Unsupported,
    Invalid,
}

impl RoomTransformUnavailable {
    pub(in crate::catalog_v3) const fn is_complete(self) -> bool {
        !matches!(self, Self::Unreadable | Self::Unsupported)
    }

    pub(in crate::catalog_v3) const fn value<T>(self) -> SourceValue<T> {
        match self {
            Self::Absent => SourceValue::Absent,
            Self::Null => SourceValue::Null,
            Self::Unreadable => SourceValue::Unreadable,
            Self::Unsupported => SourceValue::Unsupported,
            Self::Invalid => SourceValue::Invalid,
        }
    }
}

type Decoded<T> = Result<T, RoomTransformUnavailable>;

#[derive(Debug)]
pub(in crate::catalog_v3) struct RoomOffset {
    pub(in crate::catalog_v3) transform: Decoded<Transform>,
    pub(in crate::catalog_v3) inputs: Vec<SourceUse>,
}

impl<'scope, 'read, 'source> Observed<'_, 'scope, 'read, 'source> {
    pub(in crate::catalog_v3) fn room_offset(&self) -> Result<RoomOffset, CatalogError> {
        let mut inputs = vec![self.kind()?];
        let transform = self.read_room_offset(&mut inputs)?;
        Ok(RoomOffset { transform, inputs })
    }

    pub(in crate::catalog_v3) fn room_bone_input(&self) -> Result<SourceUse, CatalogError> {
        self.selector_use("boneName")?
            .map_or_else(|| self.originals.survey(self.owner), Ok)
    }

    fn read_room_offset(
        &self,
        inputs: &mut Vec<SourceUse>,
    ) -> Result<Decoded<Transform>, CatalogError> {
        let offset = match self.room_class(self.owner, "Offset", "QuatT", inputs)? {
            Ok(offset) => offset,
            Err(state) => return Ok(Err(state)),
        };
        let position = self.room_vector(offset, "Position", "Vec3", inputs)?;
        let rotation = self.room_vector(offset, "Rotation", "Ang3", inputs)?;
        Ok(position.and_then(|position| {
            rotation.and_then(|rotation| {
                p4kx_assembly::offset_transform(position, rotation)
                    .finite()
                    .ok_or(RoomTransformUnavailable::Invalid)
            })
        }))
    }

    fn room_class<'fact>(
        &'fact self,
        owner: Option<&Fact<'scope, 'read, 'source>>,
        name: &str,
        expected: &str,
        inputs: &mut Vec<SourceUse>,
    ) -> Result<Decoded<&'fact Fact<'scope, 'read, 'source>>, CatalogError> {
        let Some((_, fact)) = self.originals.field(owner, name)? else {
            inputs.push(self.originals.survey(owner)?);
            return Ok(Err(RoomTransformUnavailable::Absent));
        };
        let storage = fact.field().storage();
        if storage.data_type() != DataType::Class || storage.is_array() {
            observe_value(fact, inputs, SourceUseRole::Selector)?;
            return Ok(Err(match fact.value() {
                Value::Null => RoomTransformUnavailable::Null,
                Value::Error(_) => RoomTransformUnavailable::Unreadable,
                _ => RoomTransformUnavailable::Invalid,
            }));
        }
        let (declared, declarations) = fact.observe_declared_target(self.input)?;
        inputs.extend(declarations);
        let role = if matches!(fact.value(), Value::Error(_)) {
            SourceUseRole::Attempted
        } else {
            SourceUseRole::Selector
        };
        inputs.push(fact.observe(OriginalValuePart::Value, role)?);
        Ok(match fact.value() {
            Value::Null => Err(RoomTransformUnavailable::Null),
            Value::Error(_) => Err(RoomTransformUnavailable::Unreadable),
            Value::Struct(value)
                if declared.ok() == Some(expected) && value.type_name() == expected =>
            {
                inputs
                    .push(fact.observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector)?);
                Ok(fact)
            }
            Value::Struct(_) => Err(RoomTransformUnavailable::Unsupported),
            _ => Err(RoomTransformUnavailable::Invalid),
        })
    }

    fn room_vector(
        &self,
        offset: &Fact<'scope, 'read, 'source>,
        name: &str,
        expected: &str,
        inputs: &mut Vec<SourceUse>,
    ) -> Result<Decoded<[f32; 3]>, CatalogError> {
        let vector = match self.room_class(Some(offset), name, expected, inputs)? {
            Ok(vector) => vector,
            Err(state) => return Ok(Err(state)),
        };
        let [x, y, z] = ["x", "y", "z"];
        let x = self.room_axis(vector, x, inputs)?;
        let y = self.room_axis(vector, y, inputs)?;
        let z = self.room_axis(vector, z, inputs)?;
        Ok(x.and_then(|x| Ok([x, y?, z?])))
    }

    fn room_axis(
        &self,
        vector: &Fact<'scope, 'read, 'source>,
        name: &str,
        inputs: &mut Vec<SourceUse>,
    ) -> Result<Decoded<f32>, CatalogError> {
        let Some((_, fact)) = self.originals.field(Some(vector), name)? else {
            inputs.push(self.originals.survey(Some(vector))?);
            return Ok(Err(RoomTransformUnavailable::Absent));
        };
        observe_value(fact, inputs, SourceUseRole::Payload)?;
        let storage = fact.field().storage();
        Ok(match fact.value() {
            Value::Single(value)
                if storage.data_type() == DataType::Single
                    && !storage.is_array()
                    && value.is_finite() =>
            {
                Ok(*value)
            }
            Value::Null => Err(RoomTransformUnavailable::Null),
            Value::Error(_) => Err(RoomTransformUnavailable::Unreadable),
            _ => Err(RoomTransformUnavailable::Invalid),
        })
    }
}

fn observe_value(
    fact: &Fact<'_, '_, '_>,
    inputs: &mut Vec<SourceUse>,
    role: SourceUseRole,
) -> Result<(), CatalogError> {
    for part in [
        OriginalValuePart::FieldName,
        OriginalValuePart::DataType,
        OriginalValuePart::IsArray,
    ] {
        inputs.push(fact.observe(part, SourceUseRole::Selector)?);
    }
    let role = if matches!(fact.value(), Value::Error(_)) {
        SourceUseRole::Attempted
    } else {
        role
    };
    inputs.push(fact.observe(OriginalValuePart::Value, role)?);
    Ok(())
}
