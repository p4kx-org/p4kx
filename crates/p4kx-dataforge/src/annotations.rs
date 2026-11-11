// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use crate::{Guid, Reference, TextResolver};

/// The struct of the Records a tag Reference points to.
pub const TAG_TYPE: &str = "Tag";

/// Resolves the tag Records that Reference fields point to into their tag paths, for the writers
/// of Record XML and Record JSON.
pub trait TagResolver: Sync {
    /// Returns the tag path of the Tag Record `guid` (`Weapon.Gun`), or `None` when it is no tag
    /// the resolver knows.
    fn tag_path(&self, guid: Guid) -> Option<&str>;

    /// Returns a tag path for immediate use by an output writer.
    fn tag_path_text(&self, guid: Guid) -> Option<Cow<'_, str>> {
        self.tag_path(guid).map(Cow::Borrowed)
    }
}

/// What the writers of Record XML and Record JSON add to the plain output: the texts of
/// localisation keys and the tag paths of tag References. Without either, the output is the plain
/// Record.
#[derive(Clone, Copy, Default)]
pub struct Annotations<'a> {
    texts: Option<&'a dyn TextResolver>,
    tags: Option<&'a dyn TagResolver>,
}

impl<'a> Annotations<'a> {
    /// Creates annotations that add nothing.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            texts: None,
            tags: None,
        }
    }

    /// Adds the texts `texts` resolves next to each localisation key.
    #[must_use]
    pub const fn with_texts(mut self, texts: Option<&'a dyn TextResolver>) -> Self {
        self.texts = texts;
        self
    }

    /// Adds the tag path `tags` resolves to each tag Reference.
    #[must_use]
    pub const fn with_tags(mut self, tags: Option<&'a dyn TagResolver>) -> Self {
        self.tags = tags;
        self
    }

    /// Returns the texts resolver, if any.
    #[must_use]
    pub const fn texts(&self) -> Option<&'a dyn TextResolver> {
        self.texts
    }

    /// Returns the tag resolver, if any.
    #[must_use]
    pub const fn tags(&self) -> Option<&'a dyn TagResolver> {
        self.tags
    }

    /// Returns whether any annotation is on.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.texts.is_some() || self.tags.is_some()
    }

    /// Returns the tag path of `reference` when it is a tag Reference the resolver knows.
    pub(crate) fn tag_path(&self, reference: &Reference<'_>) -> Option<Cow<'a, str>> {
        if reference.type_name() != TAG_TYPE {
            return None;
        }
        self.tags?.tag_path_text(reference.guid())
    }
}

impl std::fmt::Debug for Annotations<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Annotations")
            .field("texts", &self.texts.is_some())
            .field("tags", &self.tags.is_some())
            .finish()
    }
}

impl<'a> From<Option<&'a dyn TextResolver>> for Annotations<'a> {
    fn from(texts: Option<&'a dyn TextResolver>) -> Self {
        Self::none().with_texts(texts)
    }
}
