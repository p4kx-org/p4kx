// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Entry;

use super::DirectoryLookup;
use crate::source_selection::EntrySelection;

impl DirectoryLookup<'_> {
    pub(crate) const fn for_definition(&self) -> Self {
        Self {
            definition_scope: true,
            ..*self
        }
    }

    pub(crate) fn selects_definition(&self, position: usize) -> bool {
        self.inputs
            .and_then(|inputs| inputs.definition_scope.get())
            .is_some_and(|scope| scope.contains_definition(position))
    }

    pub(crate) const fn uses_definition_scope(&self) -> bool {
        self.definition_scope
    }

    pub(crate) fn reference_selection(&self, entry: &Entry) -> EntrySelection {
        let original = self.selection.classify(self.chain, entry.path());
        if !self.definition_scope || original == EntrySelection::Excluded {
            return original;
        }
        if self
            .inputs
            .and_then(|inputs| inputs.definition_scope.get())
            .is_some_and(|scope| scope.contains_reference(entry.index()))
        {
            EntrySelection::Included
        } else {
            EntrySelection::Unmatched
        }
    }
}
