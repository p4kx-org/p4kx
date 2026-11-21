// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::cell::Cell;

use p4kx_gfx::{Localise, Localised, localise_text};

use super::draw::Notes;

const PLACEHOLDERS: [&str; 3] = ["LOC_EMPTY", "LOC_UNINITIALIZED", "LOC_PLACEHOLDER"];

/// The key of the label an editor gives a new text field.
///
/// On LIVE 7 labels of the 4995 Records
/// below `ui/buildingblocks` carry it, in the header canvases of the three MFD brands, and every
/// one of them is the text field whose text a binding fills (the page name, a warning, a
/// countdown); the game never shows it there.
///
/// The key is a real string of the leaderboards, so it is a placeholder only on a field that a
/// binding fills (`is_editor_label`), not for `is_placeholder`.
pub const EDITOR_LABEL: &str = "ui_leaderboards_Loadout";

/// Returns whether the label `source`, a key with its `@`, is [`EDITOR_LABEL`].
pub(crate) fn is_editor_label(source: &str) -> bool {
    source.strip_prefix('@') == Some(EDITOR_LABEL)
}

/// Returns whether `text` is empty or a key the game fills in at run time.
pub(crate) fn is_placeholder(text: &str) -> bool {
    let key = text.strip_prefix('@').unwrap_or(text);
    key.is_empty() || PLACEHOLDERS.contains(&key)
}

pub(super) struct ResolvedText<'a> {
    pub(super) text: Cow<'a, str>,
    pub(super) lookup_key: Option<Box<str>>,
}

/// Looks the localisation key `source` up, or returns it as it is, with a note why.
pub(super) fn resolve<'a>(
    localise: Option<Localise<'_>>,
    source: &'a str,
    notes: &mut Notes,
    observe: bool,
) -> ResolvedText<'a> {
    let Some(lookup) = localise else {
        if source.starts_with('@') {
            notes.push("localisation key drawn as it is: no language given".to_owned());
        }
        return ResolvedText {
            text: Cow::Borrowed(source),
            lookup_key: None,
        };
    };
    let key_used = Cell::new(None);
    let selected = |key: &str| {
        let text = lookup(key);
        if observe && text.is_some() {
            key_used.set(Some(Box::from(key)));
        }
        text
    };
    let text = match localise_text(source, &selected) {
        Localised::Plain => Cow::Borrowed(source),
        Localised::Text(text) => {
            return ResolvedText {
                text: Cow::Owned(text),
                lookup_key: key_used.take(),
            };
        }
        Localised::Missing(_) => {
            notes.push("localisation key not in the language: drawn as it is".to_owned());
            Cow::Borrowed(source)
        }
    };
    ResolvedText {
        text,
        lookup_key: None,
    }
}
