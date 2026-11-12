// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::iter;

use super::files::is_plain;

const EXTENSION: &str = ".socpak";

/// A declared Child path with its physical `.socpak` extension.
#[derive(Debug)]
pub struct ChildContainerPath<'source>(Cow<'source, str>);

impl<'source> ChildContainerPath<'source> {
    /// Returns the archive-relative lookup name for a usable authored path.
    #[must_use]
    pub fn new(name: &'source str) -> Option<Self> {
        let path = name.trim().trim_start_matches(['/', '\\']);
        if path.is_empty() || !is_plain(name) {
            return None;
        }
        let has_extension = path
            .len()
            .checked_sub(EXTENSION.len())
            .and_then(|offset| path.get(offset..))
            .is_some_and(|extension| extension.eq_ignore_ascii_case(EXTENSION));
        let lookup = if has_extension {
            Cow::Borrowed(path)
        } else {
            Cow::Owned(format!("{path}{EXTENSION}"))
        };
        Some(Self(lookup))
    }

    /// Returns the name used inside an enclosing container.
    #[must_use]
    pub fn relative(&self) -> &str {
        &self.0
    }

    /// Returns the authored lookup name and its root Archive path forms in order.
    pub fn root_candidates(&self) -> impl Iterator<Item = Cow<'_, str>> {
        let name = self.relative();
        iter::once(Cow::Borrowed(name)).chain(
            ["Data\\", "Data\\ObjectContainers\\"]
                .into_iter()
                .map(move |prefix| Cow::Owned(format!("{prefix}{name}"))),
        )
    }
}
