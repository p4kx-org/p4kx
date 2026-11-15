// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Entry;

use super::DirectoryLookup;
use crate::source_selection::EntrySelection;

#[derive(Clone, Copy)]
pub(super) enum DefinitionSelection {
    Filter,
    Scope,
    AuthoredImport,
}

impl DirectoryLookup<'_> {
    pub(crate) const fn for_definition(&self) -> Self {
        Self {
            definition_scope: DefinitionSelection::Scope,
            ..*self
        }
    }

    pub(crate) const fn for_definition_import(&self) -> Self {
        Self {
            definition_scope: match self.definition_scope {
                DefinitionSelection::Scope => DefinitionSelection::AuthoredImport,
                other => other,
            },
            ..*self
        }
    }

    pub(crate) fn selects_definition(&self, position: usize) -> bool {
        self.inputs
            .and_then(|inputs| inputs.definition_scope.get())
            .is_some_and(|scope| scope.contains_definition(position))
    }

    pub(crate) const fn uses_definition_scope(&self) -> bool {
        matches!(self.definition_scope, DefinitionSelection::Scope)
    }

    pub(crate) fn reference_selection(&self, entry: &Entry) -> EntrySelection {
        let original = self.selection.classify(self.chain, entry.path());
        if original == EntrySelection::Excluded {
            return original;
        }
        match self.definition_scope {
            DefinitionSelection::Filter => original,
            DefinitionSelection::AuthoredImport => EntrySelection::Included,
            DefinitionSelection::Scope => self.definition_target_selection(entry),
        }
    }

    fn definition_target_selection(&self, entry: &Entry) -> EntrySelection {
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
