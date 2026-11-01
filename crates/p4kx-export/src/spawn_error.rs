// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use thiserror::Error;

use crate::template_issues::TemplateError;

/// Why the spawn descriptions and location queries of a Record cannot be read.
#[derive(Debug, Error)]
pub(crate) enum SpawnError {
    /// The content of a description cannot be serialised to make its identifier.
    #[error(transparent)]
    Identifier(#[from] serde_json::Error),
    /// A location condition is of a struct the catalog does not know.
    #[error(transparent)]
    Template(#[from] TemplateError),
}
