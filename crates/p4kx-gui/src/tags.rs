// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Guid, Tag};

mod load;
mod view;

#[cfg(test)]
#[path = "tags_metadata_tests.rs"]
mod metadata_tests;

pub use view::{TagRow, TagsView};

use crate::EntryIndex;
use load::{Load, Outcome};

/// Where the tag tree of the tab stands.
#[derive(Debug, Clone, Copy)]
pub enum TagsStatus<'a> {
    /// The tags were not asked for yet.
    Idle,
    /// The tags are read off the UI thread.
    Loading,
    /// The DataForge holds no tags.
    NoTags,
    /// Reading failed for the reason given, or stopped without a result.
    Failed(Option<&'a str>),
    /// The Archive changed on disk, so the tags were not read.
    ArchiveChanged,
    /// The tree is read.
    Ready(&'a TagsView),
}

#[derive(Debug, Default)]
enum State {
    #[default]
    Idle,
    Loading(Load),
    Failed(Option<String>),
    ArchiveChanged,
    Ready(Box<TagsView>),
}

/// The Tags tab: the tag tree of the DataForge of the open Archive, which tags are expanded, the
/// search text and the selected tag.
#[derive(Debug, Default)]
pub struct TagsTab {
    state: State,
    query: String,
    generation: u64,
}

impl TagsTab {
    /// Returns the search text.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns where the tree stands.
    #[must_use]
    pub fn status(&self) -> TagsStatus<'_> {
        match &self.state {
            State::Idle => TagsStatus::Idle,
            State::Loading(_) => TagsStatus::Loading,
            State::Failed(reason) => TagsStatus::Failed(reason.as_deref()),
            State::ArchiveChanged => TagsStatus::ArchiveChanged,
            State::Ready(view) if view.tree().is_empty() && view.tree().issues().is_clean() => {
                TagsStatus::NoTags
            }
            State::Ready(view) => TagsStatus::Ready(view),
        }
    }

    /// Returns whether the tags are read or being read.
    #[must_use]
    pub const fn is_started(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    /// Starts reading the tags of `index` off the UI thread, calling `wake` from it once it
    /// ended; returns whether it started. Nothing starts when the tags are read or being read.
    pub(crate) fn load(
        &mut self,
        index: &Arc<EntryIndex>,
        wake: impl Fn() + Send + 'static,
    ) -> bool {
        if self.is_started() {
            return false;
        }
        self.state = match load::start(index, wake) {
            Ok(load) => State::Loading(load),
            Err(reason) => State::Failed(Some(reason)),
        };
        true
    }

    /// Forgets the tree, the selection and the search text, because another Archive was opened.
    pub(crate) fn archive_opened(&mut self) {
        self.state = State::Idle;
        self.query.clear();
    }

    /// Stops reading the tree and drops the one that was read, because the Archive changed on disk;
    /// nothing happens while the tags were not asked for.
    pub(crate) fn archive_changed(&mut self) {
        if self.is_started() {
            self.state = State::ArchiveChanged;
        }
    }

    /// Takes the tree once it is read; returns whether something changed.
    pub fn poll(&mut self) -> bool {
        let State::Loading(load) = &self.state else {
            return false;
        };
        let Some(outcome) = load.poll() else {
            return false;
        };
        self.generation += 1;
        self.state = match outcome {
            Outcome::Ready(tree) => {
                let mut view = TagsView::new(tree, self.generation);
                view.set_query(&self.query);
                State::Ready(Box::new(view))
            }
            Outcome::Failed(reason) => State::Failed(reason),
            Outcome::ArchiveChanged => State::ArchiveChanged,
        };
        true
    }

    /// Searches the tag paths for `text`, ignoring case; the matching tags are shown with the tags
    /// above them, however the tree is expanded.
    pub fn set_query(&mut self, text: &str) {
        text.clone_into(&mut self.query);
        if let State::Ready(view) = &mut self.state {
            view.set_query(text);
        }
    }

    /// Lets `edit` change the search text in place and searches again when it returns `true`;
    /// returns what `edit` returned.
    pub(crate) fn edit_query(&mut self, edit: impl FnOnce(&mut String) -> bool) -> bool {
        let changed = edit(&mut self.query);
        if changed && let State::Ready(view) = &mut self.state {
            view.set_query(&self.query);
        }
        changed
    }

    /// Expands the tag at `position` of the pre-order, or collapses it when it is expanded.
    pub fn toggle(&mut self, position: usize) {
        if let State::Ready(view) = &mut self.state {
            view.toggle(position);
        }
    }

    /// Selects the tag at `position` of the pre-order.
    pub fn select(&mut self, position: usize) {
        if let State::Ready(view) = &mut self.state {
            view.select(position);
        }
    }

    /// Returns the tag path of the selected tag.
    #[must_use]
    pub fn selected_path(&self) -> Option<&str> {
        self.selected().map(|tag| tag.path())
    }

    /// Returns the GUID of the selected tag.
    #[must_use]
    pub fn selected_guid(&self) -> Option<Guid> {
        self.selected().map(|tag| tag.guid())
    }

    fn selected(&self) -> Option<Tag<'_>> {
        match &self.state {
            State::Ready(view) => view.selected(),
            _ => None,
        }
    }
}
