// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Field, RecordTree, Value, ValueError};

use crate::flow_final_original::{Configuration, Declaration, Edge, Node, Original, SourceKind};
use crate::health_origin_fixture::TestResult;

struct Pending<'tree, 'source> {
    field: &'tree Field<'source>,
    value: &'tree Value<'source>,
    configuration: Option<usize>,
    declaration_ordinal: Option<u64>,
    parent: Option<usize>,
    edge: Edge,
}

pub(crate) fn capture(tree: &RecordTree<'_>) -> TestResult<Original> {
    let mut configurations = Vec::new();
    let mut pending = Vec::new();
    roots(tree, &mut configurations, &mut pending);
    let mut ordinal = 0_u64;
    while let Some(current) = pending.pop() {
        let declaration_ordinal = current.declaration_ordinal.unwrap_or(ordinal);
        let parent = if let Some(index) = current.configuration {
            let owner = configurations
                .get_mut(index)
                .ok_or("actual selected declaration")?;
            let index_in_arena = owner.nodes.len();
            owner
                .nodes
                .push(node(&current, ordinal, declaration_ordinal)?);
            Some(index_in_arena)
        } else {
            None
        };
        schedule(&mut pending, &current, declaration_ordinal, parent);
        ordinal = ordinal
            .checked_add(1)
            .ok_or("independent original ordinal overflow")?;
    }
    if configurations.is_empty() {
        configurations.push(Configuration {
            declaration_index: None,
            declaration: None,
            nodes: Vec::new(),
        });
    }
    Ok(Original {
        configurations,
        visited: ordinal,
    })
}

fn node(current: &Pending<'_, '_>, ordinal: u64, declaration_ordinal: u64) -> TestResult<Node> {
    let fields = match current.value {
        Value::Struct(value) => value.fields().iter().map(declaration).collect(),
        _ => Vec::new(),
    };
    let kind = kind(current)?;
    Ok(Node {
        ordinal,
        declaration_ordinal,
        parent: current.parent,
        edge: current.edge,
        kind,
        fields,
    })
}

fn schedule<'tree, 'source>(
    pending: &mut Vec<Pending<'tree, 'source>>,
    current: &Pending<'tree, 'source>,
    declaration_ordinal: u64,
    parent: Option<usize>,
) {
    match current.value {
        Value::Struct(value) => {
            for (index, field) in value.fields().iter().enumerate().rev() {
                pending.push(Pending {
                    field,
                    value: field.value(),
                    configuration: current.configuration,
                    declaration_ordinal: None,
                    parent,
                    edge: Edge::Field(index),
                });
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate().rev() {
                pending.push(Pending {
                    field: current.field,
                    value,
                    configuration: current.configuration,
                    declaration_ordinal: Some(declaration_ordinal),
                    parent,
                    edge: Edge::Element(index),
                });
            }
        }
        _ => {}
    }
}

fn declaration(field: &Field<'_>) -> Declaration {
    Declaration {
        name: field.name().to_owned(),
        data_type: field.storage().data_type(),
        is_array: field.storage().is_array(),
        target: field.storage().target(),
    }
}

fn kind(current: &Pending<'_, '_>) -> TestResult<SourceKind> {
    Ok(match current.value {
        Value::Struct(value) => SourceKind::Struct(value.type_name().to_owned()),
        Value::Array(_) => SourceKind::Array,
        Value::Null => SourceKind::Null,
        Value::Error(ValueError::InvalidPointer {
            struct_index,
            instance_index,
        }) => SourceKind::InvalidPointer {
            struct_index: *struct_index,
            instance_index: *instance_index,
        },
        Value::Error(_) => {
            return Err("unexpected genuine source error in this controlled fixture".into());
        }
        Value::Bool(_) => SourceKind::Scalar("boolean"),
        Value::String(_) => SourceKind::Scalar("string"),
        Value::Locale(_) => SourceKind::Scalar("locale"),
        Value::Enum(_) => SourceKind::Scalar("enum"),
        Value::Guid(_) => SourceKind::Scalar("guid"),
        Value::Reference(_) => SourceKind::Scalar(
            if current.field.storage().data_type() == DataType::Reference {
                "recordReference"
            } else {
                "weakReference"
            },
        ),
        Value::Int8(_)
        | Value::Int16(_)
        | Value::Int32(_)
        | Value::Int64(_)
        | Value::UInt8(_)
        | Value::UInt16(_)
        | Value::UInt32(_)
        | Value::UInt64(_)
        | Value::Single(_)
        | Value::Double(_) => SourceKind::Scalar("number"),
        _ => return Err("unexpected future original Value variant".into()),
    })
}

fn roots<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
    configurations: &mut Vec<Configuration>,
    pending: &mut Vec<Pending<'tree, 'source>>,
) {
    for (index, field) in tree.fields().iter().enumerate() {
        let configuration = if field.name() == "missionFlow" {
            let index_in_output = configurations.len();
            configurations.push(Configuration {
                declaration_index: Some(index),
                declaration: Some(declaration(field)),
                nodes: Vec::new(),
            });
            Some(index_in_output)
        } else {
            None
        };
        pending.push(Pending {
            field,
            value: field.value(),
            configuration,
            declaration_ordinal: None,
            parent: None,
            edge: Edge::Root,
        });
    }
    pending.reverse();
}
