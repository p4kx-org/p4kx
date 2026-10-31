// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{Guid, Reference, StructValue, Value};

use super::Rest;
use super::ops::{Arith, Cmp};
use crate::bindings::index_at;
use crate::reader::{as_struct, flag, float, int, items, reference, text};

/// The field of a canvas widget that names its canvas.
pub(super) const CANVAS_FIELD: &str = "CanvasReferenceRecord";

/// An input of a node: the node of `operations` it points at, if it is connected.
pub(crate) type Input = Option<usize>;

/// One entry of `operations`, as far as it can be evaluated at rest.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Node {
    /// A game variable: the `binding` it reads and the value it has at rest.
    Variable {
        binding: String,
        zero: Rest,
    },
    Param {
        parameter: String,
        default: Rest,
    },
    /// A localised component parameter: the text a canvas hands it, else the default.
    TextParam {
        parameter: String,
        default: String,
    },
    Invert(Input),
    All(Vec<Input>),
    Any(Vec<Input>),
    Compare {
        cmp: Cmp,
        constant: f64,
        left: Input,
        right: Input,
    },
    Switch {
        default: bool,
        exceptions: Vec<f64>,
        input: Input,
    },
    TextEmpty(Input),
    Choose {
        numbers: [f64; 2],
        input: Input,
        branches: [Input; 2],
        whole: bool,
    },
    Same(Input),
    Arith {
        op: Arith,
        amount: f64,
        left: Input,
        right: Input,
        whole: bool,
    },
    Clamp {
        bounds: [f64; 2],
        input: Input,
        inputs: [Input; 2],
    },
    /// The canvas a boolean picks: the first when it holds, else the second.
    CanvasChoice {
        canvases: [Option<Guid>; 2],
        input: Input,
    },
    /// The tag a boolean picks: a tag of the node, else the tag the node of the branch gives.
    TagChoice {
        tags: [Option<Guid>; 2],
        branches: [Input; 2],
        input: Input,
    },
    /// The tag of the first pair whose integer equals the input, else the default.
    TagSwitch {
        pairs: Vec<(f64, Option<Guid>)>,
        default: Option<Guid>,
        input: Input,
    },
    Unsupported(usize),
}

/// The kind of value a field binding sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldKind {
    Boolean,
    Number,
    Integer,
    Text,
    /// A localised text, set only where a game variable names it.
    Localised,
    Other,
}

/// A `Bindings…Field` operation: the field of a widget that the value of `input` sets.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Binding {
    pub(crate) widget: usize,
    pub(crate) field: String,
    pub(crate) kind: FieldKind,
    pub(crate) input: Input,
}

/// The `operations` of a canvas: the nodes by index and the field bindings.
#[derive(Debug, Default)]
pub(crate) struct Operations {
    pub(crate) nodes: Vec<Node>,
    pub(crate) bindings: Vec<Binding>,
    pub(crate) kinds: Vec<String>,
}

fn operation_at(structure: &StructValue<'_>, name: &str) -> Input {
    index_at(reference(structure, name)?.location(), "operations")
}

fn operations_at(structure: &StructValue<'_>, name: &str) -> Vec<Input> {
    items(structure, name)
        .iter()
        .map(|value| match value {
            Value::Reference(reference) => index_at(reference.location(), "operations"),
            _ => None,
        })
        .collect()
}

fn number(structure: &StructValue<'_>, name: &str) -> f64 {
    float(structure, name).map_or(0.0, f64::from)
}

fn field_kind(short: &str) -> Option<FieldKind> {
    match short {
        "BooleanField" => Some(FieldKind::Boolean),
        "NumberField" => Some(FieldKind::Number),
        "IntegerField" => Some(FieldKind::Integer),
        "StringField" => Some(FieldKind::Text),
        "LocalizedField" => Some(FieldKind::Localised),
        "ColorField" => Some(FieldKind::Other),
        _ => None,
    }
}

fn source(short: &str, op: &StructValue<'_>) -> Option<Node> {
    let parameter = || text(op, "parameter").unwrap_or_default().to_owned();
    let variable = |zero| Node::Variable {
        binding: text(op, "binding").unwrap_or_default().to_owned(),
        zero,
    };
    Some(match short {
        "BooleanVariable" => variable(Rest::Bool(false)),
        "NumberVariable" | "IntegerVariable" => variable(Rest::Num(0.0)),
        "StringVariable" | "LocalizedVariable" => variable(Rest::Text(String::new())),
        "BooleanComponentParameter" => Node::Param {
            parameter: parameter(),
            default: Rest::Bool(flag(op, "defaultValue").unwrap_or(false)),
        },
        "NumberComponentParameter" | "IntegerComponentParameter" => Node::Param {
            parameter: parameter(),
            default: Rest::Num(number(op, "defaultValue")),
        },
        "LocalizedComponentParameter" => Node::TextParam {
            parameter: parameter(),
            default: text(op, "defaultValue").unwrap_or_default().to_owned(),
        },
        "StringComponentParameter" => Node::Param {
            parameter: parameter(),
            default: Rest::Text(text(op, "defaultValue").unwrap_or_default().to_owned()),
        },
        _ => return None,
    })
}

fn whole(value: &Value<'_>) -> Option<f64> {
    match value {
        Value::Int32(value) => Some(f64::from(*value)),
        Value::Int16(value) => Some(f64::from(*value)),
        Value::Int8(value) => Some(f64::from(*value)),
        Value::UInt32(value) => Some(f64::from(*value)),
        Value::UInt16(value) => Some(f64::from(*value)),
        Value::UInt8(value) => Some(f64::from(*value)),
        Value::Int64(value) => i32::try_from(*value).ok().map(f64::from),
        Value::UInt64(value) => u32::try_from(*value).ok().map(f64::from),
        _ => None,
    }
}

fn node(short: &str, op: &StructValue<'_>) -> Option<Node> {
    if let Some(source) = source(short, op) {
        return Some(source);
    }
    let input = || operation_at(op, "input");
    Some(match short {
        "BooleanInvert" => Node::Invert(input()),
        "BooleanEvaluateAnd" => Node::All(operations_at(op, "inputs")),
        "BooleanEvaluateOr" => Node::Any(operations_at(op, "inputs")),
        "BooleanFromInteger" => Node::Compare {
            cmp: Cmp::parse(text(op, "type"))?,
            constant: number(op, "value"),
            left: operation_at(op, "inputL"),
            right: operation_at(op, "inputR"),
        },
        "BooleanFromNumber" => Node::Compare {
            cmp: Cmp::parse(text(op, "type"))?,
            constant: number(op, "number"),
            left: input(),
            right: operation_at(op, "inputB"),
        },
        "BooleanFromIntegerSwitch" => Node::Switch {
            default: flag(op, "defaultValue").unwrap_or(false),
            exceptions: items(op, "exceptions").iter().filter_map(whole).collect(),
            input: input(),
        },
        "BooleanFromStringIsEmpty" => Node::TextEmpty(input()),
        "StringFromBooleanCanvas" => Node::CanvasChoice {
            canvases: ["isTrue", "isFalse"].map(|name| reference(op, name).map(Reference::guid)),
            input: input(),
        },
        "TagFromBoolean" => Node::TagChoice {
            tags: ["isTrue", "isFalse"].map(|name| reference(op, name).map(Reference::guid)),
            branches: [
                operation_at(op, "inputTrue"),
                operation_at(op, "inputFalse"),
            ],
            input: input(),
        },
        "TagFromIntegerSwitch" => Node::TagSwitch {
            pairs: items(op, "values")
                .iter()
                .filter_map(as_struct)
                .map(|pair| {
                    let first = int(pair, "first")?;
                    Some((
                        f64::from(first),
                        reference(pair, "second").map(Reference::guid),
                    ))
                })
                .collect::<Option<Vec<_>>>()?,
            default: reference(op, "defaultValue").map(Reference::guid),
            input: input(),
        },
        _ => return numeric(short, op),
    })
}

fn numeric(short: &str, op: &StructValue<'_>) -> Option<Node> {
    let input = || operation_at(op, "input");
    let whole = short.starts_with("Integer");
    Some(match short {
        "NumberFromBoolean" | "IntegerFromBoolean" => Node::Choose {
            numbers: [number(op, "isTrue"), number(op, "isFalse")],
            input: input(),
            branches: [
                operation_at(op, "inputTrue"),
                operation_at(op, "inputFalse"),
            ],
            whole,
        },
        "NumberFromInteger" => Node::Same(input()),
        "NumberArithmatic" | "IntegerArithmatic" => {
            let arith = Arith::parse(text(op, "type"))?;
            let (left, right) = if whole {
                (operation_at(op, "inputL"), operation_at(op, "inputR"))
            } else {
                (input(), operation_at(op, "inputB"))
            };
            Node::Arith {
                op: arith,
                amount: number(op, "amount"),
                left,
                right,
                whole,
            }
        }
        "NumberClamp" => Node::Clamp {
            bounds: [number(op, "minValue"), number(op, "maxValue")],
            input: input(),
            inputs: [
                operation_at(op, "inputMinValue"),
                operation_at(op, "inputMaxValue"),
            ],
        },
        _ => return None,
    })
}

impl Operations {
    /// Reads the `operations` of a canvas whose scene has `scene_len` widgets before its library.
    pub(crate) fn read(root: &StructValue<'_>, scene_len: usize) -> Self {
        let mut found = Self::default();
        for operation in items(root, "operations") {
            let Some(op) = as_struct(operation) else {
                let kind = found.kind("operation");
                found.nodes.push(Node::Unsupported(kind));
                continue;
            };
            let name = op.type_name();
            let short = name.rsplit_once('_').map_or(name, |(_, short)| short);
            let short = short.strip_prefix("Bindings").unwrap_or(short);
            if let Some(kind) = field_kind(short) {
                let widget = reference(op, "widget").and_then(|widget| {
                    let location = widget.location();
                    index_at(location, "scene").or_else(|| {
                        index_at(location, "library").and_then(|at| at.checked_add(scene_len))
                    })
                });
                if let (Some(widget), Some(field)) = (widget, text(op, "field")) {
                    found.bindings.push(Binding {
                        widget,
                        field: field.to_owned(),
                        kind,
                        input: operation_at(op, "input"),
                    });
                }
                let kind = found.kind(short);
                found.nodes.push(Node::Unsupported(kind));
                continue;
            }
            let node = node(short, op).unwrap_or_else(|| Node::Unsupported(found.kind(short)));
            found.nodes.push(node);
        }
        found
    }

    /// Returns the names the game variables of the graph are bound to.
    pub(crate) fn variable_names(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter().filter_map(|node| match node {
            Node::Variable { binding, .. } => Some(binding.as_str()),
            _ => None,
        })
    }

    /// Returns the names of the game variables whose text names the canvas of a canvas widget.
    pub(crate) fn canvas_variables(&self) -> impl Iterator<Item = &str> {
        self.bindings
            .iter()
            .filter(|binding| binding.kind == FieldKind::Text && binding.field == CANVAS_FIELD)
            .filter_map(|binding| match self.nodes.get(binding.input?)? {
                Node::Variable {
                    binding,
                    zero: Rest::Text(_),
                } => Some(binding.as_str()),
                _ => None,
            })
    }

    /// Returns the canvases the graph can put in a canvas widget.
    pub(crate) fn canvas_choices(&self) -> impl Iterator<Item = Guid> + '_ {
        self.nodes
            .iter()
            .filter_map(|node| match node {
                Node::CanvasChoice { canvases, .. } => Some(canvases),
                _ => None,
            })
            .flatten()
            .flatten()
            .copied()
    }

    /// Returns the index of the kind of operation `name` among the kinds that are not evaluated.
    fn kind(&mut self, name: &str) -> usize {
        self.kinds
            .iter()
            .position(|kind| kind == name)
            .unwrap_or_else(|| {
                self.kinds.push(name.to_owned());
                self.kinds.len() - 1
            })
    }
}
