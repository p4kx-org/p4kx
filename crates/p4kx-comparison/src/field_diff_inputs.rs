// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{Field, PositionedFields, PositionedRecord, PositionedValue, Value};

use crate::RecordSide;

#[derive(Clone, Copy)]
pub(crate) struct FieldPick<'tree, 'source> {
    pub(crate) ordinal: usize,
    pub(crate) field: &'tree Field<'source>,
}

pub(crate) struct Fields<'tree, 'source> {
    values: &'tree [Field<'source>],
    positioned: Option<PositionedFields<'tree, 'source>>,
}

impl<'tree, 'source> Fields<'tree, 'source> {
    pub(crate) fn root(
        values: &'tree [Field<'source>],
        source: Option<&'tree PositionedRecord<'source>>,
    ) -> Self {
        let positioned = source
            .filter(|input| std::ptr::eq(input.tree().fields(), values))
            .and_then(|input| input.root_fields().ok());
        Self { values, positioned }
    }

    pub(crate) const fn values(&self) -> &'tree [Field<'source>] {
        self.values
    }

    pub(crate) fn node(&self, picked: FieldPick<'tree, 'source>) -> Node<'tree, 'source> {
        Node {
            value: picked.field.value(),
            site: self
                .positioned
                .as_ref()
                .and_then(|fields| fields.get(picked.ordinal)),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Node<'tree, 'source> {
    pub(crate) value: &'tree Value<'source>,
    pub(crate) site: Option<PositionedValue<'tree, 'source>>,
}

impl<'tree, 'source> Node<'tree, 'source> {
    pub(crate) fn fields(&self, values: &'tree [Field<'source>]) -> Fields<'tree, 'source> {
        let positioned = self.site.and_then(|site| site.fields().ok().flatten());
        Fields { values, positioned }
    }

    pub(crate) fn element(&self, ordinal: u32) -> Option<Self> {
        let Value::Array(values) = self.value else {
            return None;
        };
        Some(Self {
            value: values.get(usize::try_from(ordinal).ok()?)?,
            site: self.site.and_then(|site| site.element(ordinal)),
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Nodes<'tree, 'source> {
    pub(crate) old: Option<Node<'tree, 'source>>,
    pub(crate) new: Option<Node<'tree, 'source>>,
}

impl<'tree, 'source> Nodes<'tree, 'source> {
    pub(crate) const fn side(&self, side: RecordSide) -> Option<Node<'tree, 'source>> {
        match side {
            RecordSide::Old => self.old,
            RecordSide::New => self.new,
        }
    }

    pub(crate) const fn one(side: RecordSide, node: Node<'tree, 'source>) -> Self {
        match side {
            RecordSide::Old => Self {
                old: Some(node),
                new: None,
            },
            RecordSide::New => Self {
                old: None,
                new: Some(node),
            },
        }
    }
}
