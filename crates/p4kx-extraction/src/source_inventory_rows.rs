// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::{self, Debug, Formatter};
use std::sync::{Arc, OnceLock};

use serde::{Serialize, Serializer};

/// Ordered shared rows with a lazily materialized compatibility slice.
#[derive(Clone)]
pub(super) struct FrozenRows<Row>(Arc<Rows<Row>>);

struct Rows<Row> {
    shared: Vec<Arc<Row>>,
    contiguous: OnceLock<Vec<Row>>,
}

impl<Row> FrozenRows<Row> {
    pub(super) fn new(shared: Vec<Arc<Row>>) -> Self {
        Self(Arc::new(Rows {
            shared,
            contiguous: OnceLock::new(),
        }))
    }

    pub(super) fn iter(&self) -> impl ExactSizeIterator<Item = &Row> {
        self.0.shared.iter().map(Arc::as_ref)
    }

    pub(super) fn len(&self) -> usize {
        self.0.shared.len()
    }
}

impl<Row: Clone> FrozenRows<Row> {
    pub(super) fn as_slice(&self) -> &[Row] {
        self.0
            .contiguous
            .get_or_init(|| self.iter().cloned().collect())
    }
}

impl<Row: Debug> Debug for FrozenRows<Row> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}

impl<Row: PartialEq> PartialEq for FrozenRows<Row> {
    fn eq(&self, other: &Self) -> bool {
        self.0.shared == other.0.shared
    }
}

impl<Row: Eq> Eq for FrozenRows<Row> {}

impl<Row: Serialize> Serialize for FrozenRows<Row> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

#[cfg(test)]
#[path = "source_inventory_rows_tests.rs"]
mod tests;
