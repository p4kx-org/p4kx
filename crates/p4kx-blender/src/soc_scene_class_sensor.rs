// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, SourcedValue, StructValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_components::{ComponentInput, struct_type};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Sensor {
    exit_delay: SourcedValue<f64>,
    shape_type: SourcedValue<String>,
    box_shape: Option<BoxShape>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct BoxShape {
    extent: SourcedValue<Vector>,
    offset: SourcedValue<Vector>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Vector {
    x: SourcedValue<f64>,
    y: SourcedValue<f64>,
    z: SourcedValue<f64>,
}

impl Sensor {
    pub(crate) fn read(
        input: &ComponentInput<'_, '_, '_>,
        prefix: &str,
    ) -> Result<Self, SocSceneError> {
        let shape = input.component.field("SensorShape");
        let box_shape = match shape {
            Some(Value::Struct(shape)) if shape.type_name() == "SCItemProximitySensorBoxParams" => {
                let prefix = format!("{prefix}.SensorShape");
                Some(BoxShape {
                    extent: vector(input, shape, &prefix, "Extent")?,
                    offset: vector(input, shape, &prefix, "Offset")?,
                })
            }
            _ => None,
        };
        Ok(Self {
            exit_delay: input.cell(
                prefix,
                "exitDelay",
                number(input.component.field("exitDelay")),
            )?,
            shape_type: input.cell(prefix, "SensorShape", struct_type(shape))?,
            box_shape,
        })
    }
}

fn vector(
    input: &ComponentInput<'_, '_, '_>,
    shape: &StructValue<'_>,
    prefix: &str,
    name: &str,
) -> Result<SourcedValue<Vector>, SocSceneError> {
    let value = match shape.field(name) {
        Some(Value::Struct(value)) => {
            let prefix = format!("{prefix}.{name}");
            let axis = |field| input.cell(&prefix, field, number(value.field(field)));
            SourceValue::Present(Vector {
                x: axis("x")?,
                y: axis("y")?,
                z: axis("z")?,
            })
        }
        Some(Value::Null) => SourceValue::Null,
        Some(Value::Error(_)) => SourceValue::Unreadable,
        None => SourceValue::Absent,
        _ => SourceValue::Invalid,
    };
    input.cell(prefix, name, value)
}

fn number(value: Option<&Value<'_>>) -> SourceValue<f64> {
    match value {
        Some(Value::Single(number)) if number.is_finite() => {
            SourceValue::Present(f64::from(*number))
        }
        Some(Value::Double(number)) if number.is_finite() => SourceValue::Present(*number),
        Some(Value::Null) => SourceValue::Null,
        Some(Value::Error(_)) => SourceValue::Unreadable,
        None => SourceValue::Absent,
        _ => SourceValue::Invalid,
    }
}
