// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_settings::{NameProblem, SavedQueries, SavedQueryEdit};

use super::QueryTab;

/// The open dialog of the saved Queries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavedDialog {
    /// Saves the editor's Query under the name typed.
    Save {
        /// The name typed so far.
        name: String,
    },
    /// Renames the saved Query at `index`.
    Rename {
        /// The position of the saved Query.
        index: usize,
        /// The name typed so far.
        name: String,
    },
    /// Asks before the saved Query at `index` is deleted.
    Delete {
        /// The position of the saved Query.
        index: usize,
    },
}

impl SavedDialog {
    /// Returns the name typed so far for editing, `None` for the delete question.
    pub const fn name_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::Save { name } | Self::Rename { name, .. } => Some(name),
            Self::Delete { .. } => None,
        }
    }
}

/// What the dialog says about the name typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameNote {
    /// The name is free.
    Free,
    /// Saving replaces the Query saved under the name.
    Replaces,
    /// The name or the Query cannot be saved.
    Problem(NameProblem),
}

impl QueryTab {
    /// Returns the saved Queries.
    #[must_use]
    pub const fn saved(&self) -> &SavedQueries {
        &self.saved
    }

    /// Replaces the saved Queries, as read from the settings.
    pub fn set_saved(&mut self, saved: SavedQueries) {
        self.saved = saved;
        self.saved_revision = self.saved_revision.wrapping_add(1);
    }

    /// Returns a number that changes whenever the saved Queries do.
    #[must_use]
    pub const fn saved_revision(&self) -> u64 {
        self.saved_revision
    }

    /// Returns the open dialog of the saved Queries.
    #[must_use]
    pub const fn saved_dialog(&self) -> Option<&SavedDialog> {
        self.saved_dialog.as_ref()
    }

    /// Returns the open dialog of the saved Queries for typing its name.
    pub const fn saved_dialog_mut(&mut self) -> Option<&mut SavedDialog> {
        self.saved_dialog.as_mut()
    }

    /// Opens the dialog that saves the editor's Query, offering the name it was loaded under.
    pub fn begin_save(&mut self) {
        let name = self
            .loaded
            .as_deref()
            .filter(|name| self.saved.position(name).is_some())
            .unwrap_or_default()
            .to_owned();
        self.saved_dialog = Some(SavedDialog::Save { name });
    }

    /// Opens the dialog that renames the saved Query at `index`.
    pub fn begin_rename(&mut self, index: usize) {
        if let Some(saved) = self.saved.get(index) {
            self.saved_dialog = Some(SavedDialog::Rename {
                index,
                name: saved.name().to_owned(),
            });
        }
    }

    /// Opens the question before the saved Query at `index` is deleted.
    pub fn begin_delete(&mut self, index: usize) {
        if index < self.saved.len() {
            self.saved_dialog = Some(SavedDialog::Delete { index });
        }
    }

    /// Closes the dialog without a change.
    pub fn close_saved_dialog(&mut self) {
        self.saved_dialog = None;
    }

    /// Returns what the open dialog says about its name.
    #[must_use]
    pub fn name_note(&self) -> NameNote {
        match &self.saved_dialog {
            Some(SavedDialog::Save { name }) => match self.saved.check_save(name, &self.text) {
                Some(problem) => NameNote::Problem(problem),
                None if self.saved.position(name).is_some() => NameNote::Replaces,
                None => NameNote::Free,
            },
            Some(SavedDialog::Rename { index, name }) => self
                .saved
                .check_rename(*index, name)
                .map_or(NameNote::Free, NameNote::Problem),
            Some(SavedDialog::Delete { .. }) | None => NameNote::Free,
        }
    }

    /// Carries out the open dialog and closes it; returns whether the saved Queries changed.
    /// The change is also kept as an edit by name until [`Self::edits_stored`].
    pub fn submit_saved_dialog(&mut self) -> bool {
        let Some(dialog) = self.saved_dialog.take() else {
            return false;
        };
        let edit = match &dialog {
            SavedDialog::Save { name } => SavedQueryEdit::Save {
                name: name.trim().to_owned(),
                text: self.text.clone(),
            },
            SavedDialog::Rename { index, name } => SavedQueryEdit::Rename {
                from: self
                    .saved
                    .get(*index)
                    .map_or_default(|saved| saved.name().to_owned()),
                to: name.trim().to_owned(),
            },
            SavedDialog::Delete { index } => SavedQueryEdit::Delete {
                name: self
                    .saved
                    .get(*index)
                    .map_or_default(|saved| saved.name().to_owned()),
            },
        };
        let changed = self.saved.apply(&edit);
        if changed {
            self.saved_revision = self.saved_revision.wrapping_add(1);
            if let SavedQueryEdit::Save { name, .. } | SavedQueryEdit::Rename { to: name, .. } =
                &edit
            {
                self.loaded = Some(name.clone());
            }
            self.edits.push(edit);
        } else if !matches!(dialog, SavedDialog::Delete { .. }) {
            self.saved_dialog = Some(dialog);
        }
        changed
    }

    /// Returns the changes to the saved Queries not stored yet, by name and in order.
    #[must_use]
    pub fn saved_edits(&self) -> &[SavedQueryEdit] {
        &self.edits
    }

    /// Takes `stored`, the saved Queries as the settings file now holds them with this window's
    /// edits replayed, and forgets those edits.
    pub fn edits_stored(&mut self, stored: SavedQueries) {
        self.edits.clear();
        self.saved_dialog = None;
        self.set_saved(stored);
    }

    /// Puts the saved Query at `index` into the editor; returns whether there is one.
    pub fn load_saved(&mut self, index: usize) -> bool {
        let Some(saved) = self.saved.get(index) else {
            return false;
        };
        let (name, text) = (saved.name().to_owned(), saved.text().to_owned());
        self.set_text(&text);
        self.loaded = Some(name);
        true
    }
}
