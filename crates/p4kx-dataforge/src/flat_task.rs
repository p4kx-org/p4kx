// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{Field, Value};
use std::iter::Zip;
use std::ops::Range;
use std::slice::Iter;

pub(crate) enum FlatTask<'tree, 'source> {
    Fields {
        fields: &'tree [Field<'source>],
        next: usize,
        base: usize,
    },
    Value(&'tree Value<'source>),
    Items(Zip<Range<u32>, Iter<'tree, Value<'source>>>),
    RestorePath(usize),
    RestoreIndices(usize),
}
