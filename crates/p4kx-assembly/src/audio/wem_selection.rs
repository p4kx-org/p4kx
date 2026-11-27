// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// The rule that admitted a physical WEM location during Sound resolution.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum WemSelection {
    /// This Sound source has no WEM file.
    #[default]
    NotApplicable,
    /// This Sound source uses an unsupported media storage code.
    UnsupportedStorage,
    /// No eligible WEM location was found.
    NotFound,
    /// The loose WEM is in the Sound's bank language folder.
    LooseBankLanguage,
    /// The loose WEM is in the explicitly preferred language folder.
    LoosePreferredLanguage,
    /// The loose WEM is in the configured fallback language folder.
    LooseFallbackLanguage,
    /// The first localised loose WEM in path order was selected.
    LooseFirstLocalised,
    /// The loose WEM is outside the language folders.
    LooseNeutral,
    /// The WEM is embedded in the same physical bank as the Sound object.
    EmbeddedSameBank,
    /// The WEM is embedded in another bank of the same language.
    EmbeddedBankLanguage,
    /// The WEM is embedded in a bank outside the language folders.
    EmbeddedNeutral,
    /// The first available embedded WEM in bank path order was selected.
    EmbeddedFirstAvailable,
}
