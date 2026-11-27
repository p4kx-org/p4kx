// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter::Zip;
use std::ops::Range;
use std::slice::Iter;

use crate::{Field, PositionedFields, PositionedValue, Value};

pub(super) struct OriginalFlatFields<'tree, 'source> {
    pub(super) slots: PositionedFields<'tree, 'source>,
    pub(super) aliases: Vec<Option<usize>>,
}

pub(super) enum FlatTask<'tree, 'source> {
    Fields {
        fields: &'tree [Field<'source>],
        positions: Option<OriginalFlatFields<'tree, 'source>>,
        next: usize,
        base: usize,
    },
    Value {
        value: &'tree Value<'source>,
        original: Option<PositionedValue<'tree, 'source>>,
        alias: Option<PositionedValue<'tree, 'source>>,
    },
    Items {
        items: Zip<Range<u32>, Iter<'tree, Value<'source>>>,
        container: Option<PositionedValue<'tree, 'source>>,
    },
    RestorePath(usize),
    RestoreIndices(usize),
    RestoreRoute {
        aliased: bool,
    },
}
