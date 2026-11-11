// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Tag, TagTree};

/// The tag tree of the open Archive as the tab shows it: which tags are expanded, which rows the
/// search leaves, and the selected tag.
#[derive(Debug)]
pub struct TagsView {
    tree: Arc<TagTree>,
    expanded: Vec<bool>,
    shown: Vec<u32>,
    selected: Option<u32>,
    filtered: bool,
    generation: u64,
}

/// One row of the tree as the tab shows it.
#[derive(Debug, Clone, Copy)]
pub struct TagRow<'v> {
    tag: Tag<'v>,
    expandable: bool,
    expanded: bool,
    selected: bool,
}

impl<'v> TagRow<'v> {
    /// Returns the tag.
    #[must_use]
    pub const fn tag(&self) -> Tag<'v> {
        self.tag
    }

    /// Returns whether the tag has children, so that it can be expanded.
    #[must_use]
    pub const fn is_expandable(&self) -> bool {
        self.expandable
    }

    /// Returns whether the children of the tag are shown below it.
    #[must_use]
    pub const fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Returns whether the tag is the selected one.
    #[must_use]
    pub const fn is_selected(&self) -> bool {
        self.selected
    }
}

impl TagsView {
    pub(super) fn new(tree: Arc<TagTree>, generation: u64) -> Self {
        let mut view = Self {
            expanded: vec![false; tree.len()],
            shown: Vec::new(),
            selected: None,
            filtered: false,
            generation,
            tree,
        };
        view.show_expanded();
        view
    }

    /// Returns the tree.
    #[must_use]
    pub fn tree(&self) -> &TagTree {
        &self.tree
    }

    /// Returns the number of this read of the tree, which changes when another is shown.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the number of rows shown.
    #[must_use]
    pub const fn shown_len(&self) -> usize {
        self.shown.len()
    }

    /// Returns whether a search leaves only some of the tags.
    #[must_use]
    pub const fn is_filtered(&self) -> bool {
        self.filtered
    }

    /// Returns the number of places where the tree differs from what the `TagDatabase` lists.
    #[must_use]
    pub fn irregularities(&self) -> usize {
        let issues = self.tree.issues();
        issues.unclassified()
            + issues.unreadable()
            + issues.dangling()
            + issues.repeated()
            + issues.duplicate()
            + issues.unlisted()
    }

    /// Returns the row at `index` among the rows shown.
    #[must_use]
    pub fn row(&self, index: usize) -> Option<TagRow<'_>> {
        let position = *self.shown.get(index)?;
        let tag = self.tree.at(usize::try_from(position).ok()?)?;
        let expandable = !self.filtered && tag.has_children();
        let expanded = expandable && self.expanded.get(tag.position()) == Some(&true);
        Some(TagRow {
            tag,
            expandable,
            expanded,
            selected: self.selected == Some(position),
        })
    }

    /// Returns the selected tag.
    #[must_use]
    pub fn selected(&self) -> Option<Tag<'_>> {
        self.tree.at(usize::try_from(self.selected?).ok()?)
    }

    pub(super) fn select(&mut self, position: usize) {
        if let Ok(position) = u32::try_from(position)
            && self.shown.binary_search(&position).is_ok()
        {
            self.selected = Some(position);
        }
    }

    pub(super) fn toggle(&mut self, position: usize) {
        if self.filtered {
            return;
        }
        let Some(tag) = self.tree.at(position) else {
            return;
        };
        if !tag.has_children() {
            return;
        }
        if let Some(flag) = self.expanded.get_mut(position) {
            *flag = !*flag;
        }
        self.show_expanded();
    }

    pub(super) fn set_query(&mut self, text: &str) {
        let needle = text.trim();
        self.filtered = !needle.is_empty();
        if !self.filtered {
            self.show_expanded();
            return;
        }
        let lowered = needle.to_lowercase();
        let mut keep = vec![false; self.tree.len()];
        let mut path = String::new();
        for tag in self.tree.iter() {
            path.clear();
            tag.path_view().append_to(&mut path);
            if contains_ignoring_case(&path, &lowered) {
                for ancestor in tag.ancestors_and_self() {
                    match keep.get_mut(ancestor.position()) {
                        Some(flag) if !*flag => *flag = true,
                        _ => break,
                    }
                }
            }
        }
        self.shown = (0..)
            .zip(&keep)
            .filter_map(|(position, kept)| kept.then_some(position))
            .collect();
        self.drop_hidden_selection();
    }

    /// Shows every tag that is a root or below expanded tags only.
    fn show_expanded(&mut self) {
        self.shown.clear();
        let mut position = 0;
        while let Some(tag) = self.tree.at(position) {
            if let Ok(shown) = u32::try_from(position) {
                self.shown.push(shown);
            }
            let open = self.expanded.get(position) == Some(&true);
            position += 1 + if open { 0 } else { tag.descendants().len() };
        }
        self.drop_hidden_selection();
    }

    fn drop_hidden_selection(&mut self) {
        if let Some(selected) = self.selected
            && self.shown.binary_search(&selected).is_err()
        {
            self.selected = None;
        }
    }
}

/// Returns whether `haystack` holds the search text, ignoring case; `lowered` is the search text
/// in lower case.
fn contains_ignoring_case(haystack: &str, lowered: &str) -> bool {
    if haystack.is_ascii() {
        let needle = lowered.as_bytes();
        lowered.is_ascii()
            && (needle.is_empty()
                || haystack
                    .as_bytes()
                    .windows(needle.len())
                    .any(|window| window.eq_ignore_ascii_case(needle)))
    } else {
        haystack.to_lowercase().contains(lowered)
    }
}
