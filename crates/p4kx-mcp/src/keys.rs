// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::ArchiveKeyState;
use p4kx_extraction::{ArchiveKey, archive_error_in};
use p4kx_settings::{KEY_VARIABLE, KeyChoiceError, KeyStore, KeyText, choose_key};

/// Where the server takes the Archive key from: its `--key` option, the `P4KX_ARCHIVE_KEY`
/// environment variable of its configuration, then the key stored in the user's profile.
#[derive(Debug, Default, Clone)]
pub(crate) struct KeyChoice {
    flag: Option<KeyText>,
    store: Option<KeyStore>,
}

impl KeyChoice {
    pub(crate) const fn new(flag: Option<KeyText>, store: Option<KeyStore>) -> Self {
        Self { flag, store }
    }

    /// Fails when the option or the variable is set but holds no key, so the server does not start
    /// with a key it would silently ignore.
    pub(crate) fn check(&self) -> Result<(), KeyChoiceError> {
        match self.choose() {
            Err(error @ (KeyChoiceError::Flag(_) | KeyChoiceError::Environment(_))) => Err(error),
            _ => Ok(()),
        }
    }

    /// Returns the key to use now. The stored key is read each time, so a key stored while the
    /// server runs applies to the next call.
    ///
    /// # Errors
    ///
    /// Returns the [`KeyChoiceError`] when a source holds no valid key, e.g. a malformed stored
    /// key, instead of going on without a key.
    pub(crate) fn current(&self) -> Result<Option<ArchiveKey>, KeyChoiceError> {
        Ok(self.choose()?.map(|chosen| chosen.key))
    }

    fn choose(&self) -> Result<Option<p4kx_settings::ChosenKey>, KeyChoiceError> {
        let variable = std::env::var(KEY_VARIABLE).ok();
        choose_key(
            self.flag.as_ref().map(KeyText::as_str),
            variable.as_deref(),
            self.store.as_ref(),
        )
    }
}

const KEY_HINT: &str = "The server has no Archive key, so it cannot read encrypted Entries. Start the server \
with --key <hex>, set P4KX_ARCHIVE_KEY in its configuration, or store the key once with \
`p4kx key set` or in the GUI.";
const WRONG_KEY_HINT: &str = "The server's Archive key does not fit this Archive. Correct --key or \
P4KX_ARCHIVE_KEY, or replace the stored key with `p4kx key set` or in the GUI.";

/// Returns how to give the server the key when `state` says encrypted Entries stayed unread.
pub(crate) const fn state_hint(state: ArchiveKeyState) -> Option<&'static str> {
    match state {
        ArchiveKeyState::Given => None,
        ArchiveKeyState::Missing => Some(KEY_HINT),
        ArchiveKeyState::Wrong => Some(WRONG_KEY_HINT),
    }
}

/// Returns how to give the server the key when `error` is about a missing or wrong Archive key.
pub(crate) fn hint(error: &(dyn std::error::Error + 'static)) -> Option<&'static str> {
    let found = archive_error_in(error)?;
    if found.is_key_required() {
        Some(KEY_HINT)
    } else if found.is_wrong_key() {
        Some(WRONG_KEY_HINT)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_holds_no_key_text() {
        let choice = KeyChoice::new(Some(KeyText::new("00112233".repeat(4))), None);

        let shown = format!("{choice:?} {choice:#?}");

        assert!(!shown.contains("00112233"), "{shown}");
        assert!(!shown.contains("eeff"), "{shown}");
    }
}
