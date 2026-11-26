// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Cell<T> {
    pub(super) state: SourceValueState,
    pub(super) value: Option<T>,
    pub(super) contains_encrypted_source: Option<bool>,
}

impl<T> Cell<T> {
    pub(super) fn map<U>(self, project: impl FnOnce(T) -> U) -> Cell<U> {
        Cell {
            state: self.state,
            value: self.value.map(project),
            contains_encrypted_source: self.contains_encrypted_source,
        }
    }

    pub(super) fn try_map<U, E>(
        self,
        project: impl FnOnce(T) -> Result<U, E>,
    ) -> Result<Cell<U>, E> {
        Ok(Cell {
            state: self.state,
            value: self.value.map(project).transpose()?,
            contains_encrypted_source: self.contains_encrypted_source,
        })
    }
}

impl<'source, T> From<&'source SourcedValue<T>> for Cell<&'source T> {
    fn from(source: &'source SourcedValue<T>) -> Self {
        Self {
            state: source.state(),
            value: source.value(),
            contains_encrypted_source: source.origin().contains_encrypted_source(),
        }
    }
}

pub(super) const fn merge_encryption(left: Option<bool>, right: Option<bool>) -> Option<bool> {
    match (left, right) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), Some(false)) => Some(false),
        _ => None,
    }
}
