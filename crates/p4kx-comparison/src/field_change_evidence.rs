// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{PositionedSources, RecordJsonBytes, RecordJsonSource};
use p4kx_extraction::SourceUseRole;

use crate::field_diff_inputs::Nodes;
use crate::{RecordSide, Value};

#[derive(Clone, Copy)]
pub(crate) enum PathStep {
    Field(RecordSide),
    Element,
}

pub(crate) struct RouteStep<'tree, 'source> {
    pub(crate) path: PathStep,
    pub(crate) nodes: Nodes<'tree, 'source>,
}

#[derive(Debug, Clone)]
pub(crate) struct FieldEvidence {
    pub(crate) path: Coordinates,
    pub(crate) old: Option<OperandEvidence>,
    pub(crate) new: Option<OperandEvidence>,
}

impl FieldEvidence {
    pub(crate) fn capture(route: &[RouteStep<'_, '_>]) -> Self {
        let path = capture_path(route);
        let old = capture_operand(route, RecordSide::Old);
        let new = capture_operand(route, RecordSide::New);
        Self { path, old, new }
    }
}

fn capture_path(route: &[RouteStep<'_, '_>]) -> Coordinates {
    let mut path = Coordinates::complete();
    for step in route {
        match step.path {
            PathStep::Field(side) => {
                let names = step
                    .nodes
                    .side(side)
                    .and_then(|node| node.site)
                    .and_then(|site| site.field_name_sources());
                path.observe(side, names.as_ref(), name_role);
            }
            PathStep::Element => {
                for side in [RecordSide::Old, RecordSide::New] {
                    if let Some(node) = step.nodes.side(side) {
                        let header = node.site.and_then(|site| site.array_header_source());
                        path.require(side, header, SourceUseRole::Selector);
                    }
                }
            }
        }
    }
    path
}

const fn name_role(coordinate: RecordJsonSource<'_>) -> SourceUseRole {
    match coordinate {
        RecordJsonSource::PropertyName { .. } => SourceUseRole::Selector,
        _ => SourceUseRole::Conversion,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct OperandEvidence {
    pub(crate) route: Coordinates,
    pub(crate) kind: Coordinates,
    pub(crate) value: Coordinates,
}

fn capture_operand(route: &[RouteStep<'_, '_>], side: RecordSide) -> Option<OperandEvidence> {
    let (last, ancestors) = route.split_last()?;
    let operand = last.nodes.side(side)?;
    let mut chosen = Coordinates::complete();
    for ancestor in ancestors {
        let selection = ancestor
            .nodes
            .side(side)
            .and_then(|node| node.site)
            .and_then(|site| site.selection_sources());
        chosen.observe(side, selection.as_ref(), selection_role);
    }
    let mut kind = Coordinates::complete();
    let value_sources = operand.site.map(|site| site.value_sources());
    let struct_sources = match (operand.value, operand.site) {
        (Value::Struct(_), Some(location)) => location.selection_sources(),
        _ => None,
    };
    let kind_sources = struct_sources.as_ref().or(value_sources.as_ref());
    if declaration_is_enough(operand.value) {
        kind.observe_matching(side, kind_sources, selection_role, |coordinate| {
            matches!(coordinate, RecordJsonSource::PropertyStorage { .. })
        });
    } else {
        kind.observe(side, kind_sources, selection_role);
    }

    let mut value = Coordinates::complete();
    value.observe(side, value_sources.as_ref(), |coordinate| {
        value_role(operand.value, coordinate)
    });
    Some(OperandEvidence {
        route: chosen,
        kind,
        value,
    })
}

const fn declaration_is_enough(value: &Value<'_>) -> bool {
    matches!(
        value,
        Value::Int8(_)
            | Value::Int16(_)
            | Value::Int32(_)
            | Value::Int64(_)
            | Value::UInt8(_)
            | Value::UInt16(_)
            | Value::UInt32(_)
            | Value::UInt64(_)
            | Value::Single(_)
            | Value::Double(_)
            | Value::Guid(_)
    )
}

const fn selection_role(coordinate: RecordJsonSource<'_>) -> SourceUseRole {
    match coordinate {
        RecordJsonSource::PropertyStorage { .. } => SourceUseRole::Conversion,
        _ => SourceUseRole::Selector,
    }
}

const fn value_role(value: &Value<'_>, coordinate: RecordJsonSource<'_>) -> SourceUseRole {
    match (value, coordinate) {
        (_, RecordJsonSource::PropertyStorage { .. }) | (Value::Error(_), _) => {
            SourceUseRole::Conversion
        }
        (
            Value::Array(_),
            RecordJsonSource::Bytes {
                kind: RecordJsonBytes::ArrayHeader,
                ..
            },
        ) => SourceUseRole::Conversion,
        (
            _,
            RecordJsonSource::Bytes {
                kind: RecordJsonBytes::ArrayHeader,
                ..
            },
        ) => SourceUseRole::Selector,
        (
            _,
            RecordJsonSource::StructName { .. }
            | RecordJsonSource::Bytes {
                kind: RecordJsonBytes::Text,
                ..
            },
        ) => SourceUseRole::Payload,
        (Value::Struct(_) | Value::String(_) | Value::Locale(_) | Value::Enum(_), _) => {
            SourceUseRole::Selector
        }
        _ => SourceUseRole::Payload,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Coordinates {
    pub(crate) uses: Vec<Observation>,
    pub(crate) complete: bool,
}

impl Coordinates {
    const fn complete() -> Self {
        Self {
            uses: Vec::new(),
            complete: true,
        }
    }

    fn observe(
        &mut self,
        side: RecordSide,
        observations: Option<&PositionedSources>,
        role: impl Fn(RecordJsonSource<'_>) -> SourceUseRole,
    ) {
        self.observe_matching(side, observations, role, |_| true);
    }

    fn observe_matching(
        &mut self,
        side: RecordSide,
        observations: Option<&PositionedSources>,
        role: impl Fn(RecordJsonSource<'_>) -> SourceUseRole,
        keep: impl Fn(RecordJsonSource<'_>) -> bool,
    ) {
        if let Some(source) = observations {
            self.complete &= source.complete();
            self.uses.extend(
                source
                    .sources()
                    .iter()
                    .copied()
                    .filter(|source| keep(*source))
                    .map(|coordinate| Observation {
                        side,
                        coordinate,
                        role: role(coordinate),
                    }),
            );
        } else {
            self.complete = false;
        }
    }

    fn require(
        &mut self,
        side: RecordSide,
        coordinate: Option<RecordJsonSource<'static>>,
        role: SourceUseRole,
    ) {
        self.complete &= coordinate.is_some();
        self.uses.extend(coordinate.map(|coordinate| Observation {
            side,
            coordinate,
            role,
        }));
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Observation {
    pub(crate) side: RecordSide,
    pub(crate) coordinate: RecordJsonSource<'static>,
    pub(crate) role: SourceUseRole,
}
