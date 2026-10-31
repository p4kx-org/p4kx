// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_gfx::{Localise, Localised, localise_text};

use super::draw::Notes;

const PLACEHOLDERS: [&str; 3] = ["LOC_EMPTY", "LOC_UNINITIALIZED", "LOC_PLACEHOLDER"];

/// Returns whether `text` is empty or a key the game fills in at run time.
pub(crate) fn is_placeholder(text: &str) -> bool {
    let key = text.strip_prefix('@').unwrap_or(text);
    key.is_empty() || PLACEHOLDERS.contains(&key)
}

/// Looks the localisation key `source` up, or returns it as it is, with a note why.
pub(super) fn resolve<'a>(
    localise: Option<Localise<'_>>,
    source: &'a str,
    notes: &mut Notes,
) -> Cow<'a, str> {
    let Some(lookup) = localise else {
        if source.starts_with('@') {
            notes.push("localisation key drawn as it is: no language given".to_owned());
        }
        return Cow::Borrowed(source);
    };
    match localise_text(source, lookup) {
        Localised::Plain => Cow::Borrowed(source),
        Localised::Text(text) => Cow::Owned(text),
        Localised::Missing(_) => {
            notes.push("localisation key not in the language: drawn as it is".to_owned());
            Cow::Borrowed(source)
        }
    }
}
