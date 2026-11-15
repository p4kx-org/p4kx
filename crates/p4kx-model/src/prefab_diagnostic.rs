// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::SourceElementPosition;
use thiserror::Error;

use crate::PrefabError;

/// The original value or derived fact retained in a failed Prefab parse message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrefabDiagnostic {
    /// The rejected original XML root name and its available node position.
    RootName(Option<SourceElementPosition>),
}

/// A Prefab parse failure with the source of its retained diagnostic content.
#[derive(Debug, Error)]
#[error("{error}")]
pub struct PrefabParseFailure {
    #[source]
    error: PrefabError,
    diagnostic: Option<PrefabDiagnostic>,
}

impl PrefabParseFailure {
    /// Returns the original parser error with its unchanged display.
    #[must_use]
    pub fn into_error(self) -> PrefabError {
        self.error
    }

    /// Returns the source of a value printed in this parser diagnostic.
    #[must_use]
    pub fn into_diagnostic(self) -> Option<PrefabDiagnostic> {
        self.diagnostic
    }

    pub(crate) fn root_name(found: &str, position: Option<SourceElementPosition>) -> Self {
        Self {
            error: PrefabError::NotLibrary {
                found: found.to_owned(),
            },
            diagnostic: Some(PrefabDiagnostic::RootName(position)),
        }
    }
}

impl From<PrefabError> for PrefabParseFailure {
    fn from(error: PrefabError) -> Self {
        let diagnostic = match &error {
            PrefabError::NotLibrary { .. } => Some(PrefabDiagnostic::RootName(None)),
            PrefabError::Xml(_)
            | PrefabError::CryXml(_)
            | PrefabError::TooDeep(_)
            | PrefabError::TooMany { .. } => None,
        };
        Self { error, diagnostic }
    }
}
