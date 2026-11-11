// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Field, StructValue, Value};

use super::owned_walk::WalkFailure;
use super::path::PathMark;
use super::tokens::{OriginalContainerKind, OriginalValueEdge};

#[derive(Clone, Copy)]
pub(super) struct OwnerPosition<'source> {
    pub(super) type_name: &'source str,
    pub(super) parent_type: Option<&'source str>,
}

pub(super) enum DeclarationSource {
    Field,
    Element { ordinal: u64 },
}

pub(super) struct Child<'read, 'source> {
    pub(super) field: &'read Field<'source>,
    pub(super) value: &'read Value<'source>,
    pub(super) edge: OriginalValueEdge,
    pub(super) declaration: DeclarationSource,
    pub(super) parent: Option<u64>,
    pub(super) owner: OwnerPosition<'source>,
}

enum Contents<'read, 'source> {
    Fields {
        source: &'read StructValue<'source>,
    },
    Elements {
        field: &'read Field<'source>,
        values: &'read [Value<'source>],
        declaration_ordinal: u64,
    },
}

/// A borrowed container whose next edge is read from the actual canonical source.
pub(super) struct Frame<'read, 'source> {
    contents: Contents<'read, 'source>,
    next: usize,
    pub(super) ordinal: Option<u64>,
    pub(super) restore: PathMark,
    owner: OwnerPosition<'source>,
}

impl<'read, 'source> Frame<'read, 'source> {
    pub(super) fn root(source: &'read StructValue<'source>) -> Self {
        Self::structure(source, None, PathMark::default(), None)
    }

    pub(super) const fn structure(
        source: &'read StructValue<'source>,
        ordinal: Option<u64>,
        restore: PathMark,
        parent_type: Option<&'source str>,
    ) -> Self {
        Self {
            contents: Contents::Fields { source },
            next: 0,
            ordinal,
            restore,
            owner: OwnerPosition {
                type_name: source.type_name(),
                parent_type,
            },
        }
    }

    pub(super) const fn array(
        field: &'read Field<'source>,
        values: &'read [Value<'source>],
        ordinal: u64,
        restore: PathMark,
        owner: OwnerPosition<'source>,
        declaration_ordinal: u64,
    ) -> Self {
        Self {
            contents: Contents::Elements {
                field,
                values,
                declaration_ordinal,
            },
            next: 0,
            ordinal: Some(ordinal),
            restore,
            owner,
        }
    }

    pub(super) fn next_child(&mut self) -> Result<Option<Child<'read, 'source>>, WalkFailure> {
        let index = self.next;
        let ordinal = u64::try_from(index).map_err(|_| WalkFailure::CounterOverflow)?;
        let (field, value, edge, declaration) = match &self.contents {
            Contents::Fields { source } => {
                let source = *source;
                let Some(field) = source.fields().get(index) else {
                    return Ok(None);
                };
                (
                    field,
                    field.value(),
                    OriginalValueEdge::Field { ordinal },
                    DeclarationSource::Field,
                )
            }
            Contents::Elements {
                field,
                values,
                declaration_ordinal,
            } => {
                let values = *values;
                let Some(value) = values.get(index) else {
                    return Ok(None);
                };
                (
                    *field,
                    value,
                    OriginalValueEdge::Element { ordinal },
                    DeclarationSource::Element {
                        ordinal: *declaration_ordinal,
                    },
                )
            }
        };
        self.next = index.checked_add(1).ok_or(WalkFailure::CounterOverflow)?;
        Ok(Some(Child {
            field,
            value,
            edge,
            declaration,
            parent: self.ordinal,
            owner: self.owner,
        }))
    }

    pub(super) const fn kind(&self) -> OriginalContainerKind {
        match self.contents {
            Contents::Fields { .. } => OriginalContainerKind::Struct,
            Contents::Elements { .. } => OriginalContainerKind::Array,
        }
    }
}
