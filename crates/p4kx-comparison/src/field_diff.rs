// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;

use p4kx_dataforge::PositionedRecord;

use crate::field_change_evidence::{FieldEvidence, PathStep, RouteStep};
use crate::field_diff_inputs::{FieldPick, Fields, Node, Nodes};
use crate::{ChangedValue, Field, FieldChange, Location, RecordSide, Step, Value};

/// Returns changed or one-sided fields in new layout order, including nested values.
pub(crate) fn diff_fields<'tree, 'source>(
    old: &'tree [Field<'source>],
    new: &'tree [Field<'source>],
    old_positions: Option<&'tree PositionedRecord<'source>>,
    new_positions: Option<&'tree PositionedRecord<'source>>,
) -> Vec<FieldChange<'source>> {
    let mut walk = Walk {
        route: (old_positions.is_some() || new_positions.is_some()).then(Vec::new),
        ..Walk::default()
    };
    walk.fields(
        &Fields::root(old, old_positions),
        &Fields::root(new, new_positions),
    );
    walk.changes
}

#[derive(Default)]
struct Walk<'tree, 'source> {
    steps: Vec<Step<'source>>,
    route: Option<Vec<RouteStep<'tree, 'source>>>,
    changes: Vec<FieldChange<'source>>,
}

impl<'tree, 'source> Walk<'tree, 'source> {
    fn fields(&mut self, old: &Fields<'tree, 'source>, new: &Fields<'tree, 'source>) {
        let old_fields = old.values();
        let new_fields = new.values();
        let same_layout = old_fields.len() == new_fields.len()
            && old_fields
                .iter()
                .zip(new_fields)
                .all(|(a, b)| a.name() == b.name());
        if same_layout {
            for (ordinal, (before, after)) in old_fields.iter().zip(new_fields).enumerate() {
                self.at(
                    Step::Field(before.name()),
                    PathStep::Field(RecordSide::Old),
                    Nodes {
                        old: Some(old.node(FieldPick {
                            ordinal,
                            field: before,
                        })),
                        new: Some(new.node(FieldPick {
                            ordinal,
                            field: after,
                        })),
                    },
                );
            }
        } else {
            for (before, after) in join_by_name(old_fields, new_fields) {
                let side = if after.is_some() {
                    RecordSide::New
                } else {
                    RecordSide::Old
                };
                let name = after.or(before).map_or("", |picked| picked.field.name());
                self.at(
                    Step::Field(name),
                    PathStep::Field(side),
                    Nodes {
                        old: before.map(|picked| old.node(picked)),
                        new: after.map(|picked| new.node(picked)),
                    },
                );
            }
        }
    }

    fn at(&mut self, step: Step<'source>, path: PathStep, nodes: Nodes<'tree, 'source>) {
        self.steps.push(step);
        if let Some(route) = &mut self.route {
            route.push(RouteStep { path, nodes });
        }
        self.value(nodes);
        self.steps.pop();
        if let Some(route) = &mut self.route {
            route.pop();
        }
    }

    fn value(&mut self, nodes: Nodes<'tree, 'source>) {
        match (nodes.old, nodes.new) {
            (Some(before), Some(after)) => self.matched(nodes, before, after),
            _ => self.changed(nodes),
        }
    }

    fn matched(
        &mut self,
        nodes: Nodes<'tree, 'source>,
        before: Node<'tree, 'source>,
        after: Node<'tree, 'source>,
    ) {
        match (before.value, after.value) {
            (Value::Struct(old), Value::Struct(new)) => {
                if old.type_name() != new.type_name() {
                    self.push(nodes);
                }
                self.fields(&before.fields(old.fields()), &after.fields(new.fields()));
            }
            (Value::Array(old), Value::Array(new)) => {
                for (ordinal, _) in (0_u32..).zip(0..old.len().max(new.len())) {
                    self.at(
                        Step::Element(ordinal),
                        PathStep::Element,
                        Nodes {
                            old: before.element(ordinal),
                            new: after.element(ordinal),
                        },
                    );
                }
            }
            (old, new) if same(old, new) => {}
            _ => self.changed(nodes),
        }
    }

    fn changed(&mut self, nodes: Nodes<'tree, 'source>) {
        self.push(nodes);
        for side in [RecordSide::Old, RecordSide::New] {
            if let Some(node) = nodes.side(side) {
                self.one_side(node, side);
            }
        }
    }

    fn one_side(&mut self, node: Node<'tree, 'source>, side: RecordSide) {
        match node.value {
            Value::Struct(structure) => {
                let fields = node.fields(structure.fields());
                for (ordinal, field) in fields.values().iter().enumerate() {
                    let selected = fields.node(FieldPick { ordinal, field });
                    self.at(
                        Step::Field(field.name()),
                        PathStep::Field(side),
                        Nodes::one(side, selected),
                    );
                }
            }
            Value::Array(items) => {
                for (ordinal, _) in (0_u32..).zip(items.iter()) {
                    if let Some(selected) = node.element(ordinal) {
                        self.at(
                            Step::Element(ordinal),
                            PathStep::Element,
                            Nodes::one(side, selected),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn push(&mut self, nodes: Nodes<'tree, 'source>) {
        self.changes.push(FieldChange {
            path: Location::new(&self.steps),
            old: nodes.old.map(|node| ChangedValue::of(node.value)),
            new: nodes.new.map(|node| ChangedValue::of(node.value)),
            evidence: self
                .route
                .as_deref()
                .map(FieldEvidence::capture)
                .map(Box::new),
        });
    }
}

type Pair<'tree, 'source> = (
    Option<FieldPick<'tree, 'source>>,
    Option<FieldPick<'tree, 'source>>,
);

/// Joins fields by name and original ordinal and preserves the display order.
fn join_by_name<'tree, 'source>(
    old: &'tree [Field<'source>],
    new: &'tree [Field<'source>],
) -> Vec<Pair<'tree, 'source>> {
    let (old_names, new_names) = (by_name(old), by_name(new));
    let mut ordered: Vec<(usize, Pair<'tree, 'source>)> =
        Vec::with_capacity(old.len().max(new.len()));
    let (mut i, mut j) = (0, 0);
    loop {
        let (before, after) = (old_names.get(i), new_names.get(j));
        let order = match (before, after) {
            (None, None) => break,
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(before), Some(after)) => before.field.name().cmp(after.field.name()),
        };
        match (order, before, after) {
            (Ordering::Less, Some(&field), _) => {
                i += 1;
                ordered.push((new.len().saturating_add(field.ordinal), (Some(field), None)));
            }
            (Ordering::Greater, _, Some(&field)) => {
                j += 1;
                ordered.push((field.ordinal, (None, Some(field))));
            }
            (_, Some(&a), Some(&b)) => {
                i += 1;
                j += 1;
                ordered.push((b.ordinal, (Some(a), Some(b))));
            }
            _ => break,
        }
    }
    ordered.sort_unstable_by_key(|(index, _)| *index);
    ordered.into_iter().map(|(_, pair)| pair).collect()
}

fn by_name<'tree, 'source>(fields: &'tree [Field<'source>]) -> Vec<FieldPick<'tree, 'source>> {
    let mut named: Vec<_> = fields
        .iter()
        .enumerate()
        .map(|(ordinal, field)| FieldPick { ordinal, field })
        .collect();
    named.sort_unstable_by(|a, b| {
        a.field
            .name()
            .cmp(b.field.name())
            .then(a.ordinal.cmp(&b.ordinal))
    });
    named
}

/// Returns whether two values that are not both structs or both arrays are the same.
///
/// Floats compare by their bits, every `NaN` equal to every other, so `-0.0` differs from `0.0`.
/// References compare by target GUID and, for a weak pointer, by the location of the target;
/// values that cannot be read compare by their reason.
fn same(old: &Value<'_>, new: &Value<'_>) -> bool {
    match (old, new) {
        (Value::Single(a), Value::Single(b)) => {
            (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
        }
        (Value::Double(a), Value::Double(b)) => {
            (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
        }
        (Value::Reference(a), Value::Reference(b)) => {
            a.guid() == b.guid() && a.location() == b.location()
        }
        (Value::Struct(_) | Value::Array(_), _) | (_, Value::Struct(_) | Value::Array(_)) => false,
        _ => old == new,
    }
}
