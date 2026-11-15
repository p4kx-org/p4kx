// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::rc::Rc;

use p4kx_dataforge::{Location, Reference, Step, StructValue, Value};

use crate::reader::{as_struct, flag, float, items, reference, text};
use crate::rest::{Rest, RestParams};

const PARAMETER: &str = "BuildingBlocks_BindingsLocalizedComponentParameter";
const FIELD: &str = "BuildingBlocks_BindingsLocalizedField";
const INPUT: &str = "BuildingBlocks_ComponentParameterInputLocalization";

/// The component parameter that fills the text of a widget, with the text it has when the
/// canvas that holds the component names no value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextBinding {
    pub(crate) parameter: String,
    pub(crate) default: Option<String>,
}

/// The localised values a `WidgetCanvas` hands to the component parameters of its canvas.
pub(crate) type ParamValues = Rc<[(String, String)]>;

pub(crate) fn index_at(location: Option<&Location<'_>>, field: &str) -> Option<usize> {
    match location.map(Location::steps) {
        Some([Step::Field(name), Step::Element(index)]) if *name == field => {
            usize::try_from(*index).ok()
        }
        _ => None,
    }
}

/// Reads which widget texts a canvas binds to component parameters.
pub(crate) fn text_bindings(
    root: &StructValue<'_>,
    find_widget: &impl Fn(&Reference<'_>) -> Option<usize>,
) -> HashMap<usize, TextBinding> {
    let operations = items(root, "operations");
    let parameters: HashMap<usize, TextBinding> = operations
        .iter()
        .enumerate()
        .filter_map(|(at, operation)| {
            let operation = as_struct(operation).filter(|op| op.type_name() == PARAMETER)?;
            Some((
                at,
                TextBinding {
                    parameter: text(operation, "parameter")?.to_owned(),
                    default: text(operation, "defaultValue").map(str::to_owned),
                },
            ))
        })
        .collect();
    operations
        .iter()
        .filter_map(as_struct)
        .filter(|operation| {
            operation.type_name() == FIELD && text(operation, "field") == Some("Text")
        })
        .filter_map(|operation| {
            let widget = find_widget(reference(operation, "widget")?)?;
            let input = index_at(reference(operation, "input")?.location(), "operations")?;
            Some((widget, parameters.get(&input)?.clone()))
        })
        .collect()
}

/// Reads the boolean, number, integer and text parameter values a widget hands to its nested
/// canvas.
pub(crate) fn typed_values(widget: &StructValue<'_>) -> RestParams {
    items(widget, "paramInputValues")
        .iter()
        .filter_map(|value| {
            let Value::Struct(input) = value else {
                return None;
            };
            let parameter = text(input, "parameter")?.to_owned();
            let rest = match input.type_name() {
                "BuildingBlocks_ComponentParameterInputBoolean" => {
                    Rest::Bool(flag(input, "value")?)
                }
                "BuildingBlocks_ComponentParameterInputInteger"
                | "BuildingBlocks_ComponentParameterInputNumber" => {
                    Rest::Num(f64::from(float(input, "value").filter(|v| v.is_finite())?))
                }
                "BuildingBlocks_ComponentParameterInputString" => {
                    Rest::Text(text(input, "value")?.to_owned())
                }
                _ => return None,
            };
            Some((parameter, rest))
        })
        .collect()
}

/// Reads the localised parameter values a widget hands to its nested canvas.
pub(crate) fn param_values(widget: &StructValue<'_>) -> ParamValues {
    items(widget, "paramInputValues")
        .iter()
        .filter_map(|value| match value {
            Value::Struct(input) if input.type_name() == INPUT => Some((
                text(input, "parameter")?.to_owned(),
                text(input, "value")?.to_owned(),
            )),
            _ => None,
        })
        .collect()
}
