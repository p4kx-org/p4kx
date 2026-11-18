// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::source_fields::FieldError;

pub(crate) struct ScopedSourceRead<T> {
    pub(crate) value: T,
    pub(crate) notes: Vec<FieldError>,
    pub(crate) selection_complete: bool,
}

impl<T> ScopedSourceRead<T> {
    pub(crate) const fn new(value: T, notes: Vec<FieldError>, selection_complete: bool) -> Self {
        Self {
            value,
            notes,
            selection_complete,
        }
    }

    pub(crate) fn map<U>(self, convert: impl FnOnce(T) -> U) -> ScopedSourceRead<U> {
        ScopedSourceRead {
            value: convert(self.value),
            notes: self.notes,
            selection_complete: self.selection_complete,
        }
    }
}
