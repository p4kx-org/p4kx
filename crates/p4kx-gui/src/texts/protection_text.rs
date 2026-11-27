// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A localised output-protection or runtime-prerequisite message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProtectionText {
    /// The destination's host identity is unavailable.
    TargetIdentity,
    /// The source Archive's host identity is unavailable.
    SourceIdentity,
    /// The source parent's installation membership is unavailable; takes `parent`.
    SourceMembership,
    /// An output would overwrite an input Archive.
    SourceOverwrite,
    /// The desktop executable command-line help.
    CommandHelp,
    /// An output identity could not be accepted.
    IdentityUnavailable,
    /// The title of the Flatpak prerequisite failure.
    PrerequisiteTitle,
    /// The required Documents portal and available recovery actions.
    Prerequisite,
}

impl ProtectionText {
    pub(crate) const ALL: [Self; 8] = [
        Self::TargetIdentity,
        Self::SourceIdentity,
        Self::SourceMembership,
        Self::SourceOverwrite,
        Self::CommandHelp,
        Self::IdentityUnavailable,
        Self::PrerequisiteTitle,
        Self::Prerequisite,
    ];

    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::TargetIdentity => "protection-target-identity",
            Self::SourceIdentity => "protection-source-identity",
            Self::SourceMembership => "protection-source-membership",
            Self::SourceOverwrite => "protection-source-overwrite",
            Self::CommandHelp => "gui-command-help",
            Self::IdentityUnavailable => "protection-identity-unavailable",
            Self::PrerequisiteTitle => "protection-prerequisite-title",
            Self::Prerequisite => "protection-prerequisite",
        }
    }
}

impl From<ProtectionText> for crate::Text {
    fn from(text: ProtectionText) -> Self {
        Self::Protection(text)
    }
}
