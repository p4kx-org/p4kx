// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use thiserror::Error;

/// Why the latest Release could not be determined.
#[derive(Debug, Error)]
pub enum UpdateError {
    /// The embedded native target contract is inconsistent.
    #[error(transparent)]
    Platform(#[from] crate::PlatformError),
    /// The request failed or was answered with an error status.
    #[error("cannot ask {endpoint} for the latest Release")]
    Request {
        /// The endpoint asked.
        endpoint: String,
        /// The cause.
        #[source]
        source: Box<ureq::Error>,
    },
    /// The answer is not the JSON of a Release.
    #[error("the answer of {endpoint} is not a Release")]
    Malformed {
        /// The endpoint asked.
        endpoint: String,
        /// The cause.
        #[source]
        source: serde_json::Error,
    },
    /// A version is not a semantic version.
    #[error("{version} is not a version")]
    Version {
        /// The text that was read as a version.
        version: String,
        /// The cause.
        #[source]
        source: semver::Error,
    },
}
