// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::discovery_limits::DiscoveryLimit;

/// A bounded discovery operation that could not establish an ordinary result.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum DiscoveryIssueKind {
    /// The source operation failed.
    Io(Arc<io::Error>),
    /// More than one component matched after ASCII case folding.
    Ambiguous,
    /// A configured work limit was reached.
    Limit(DiscoveryLimit),
    /// A configured Steam library list was malformed.
    MalformedConfig,
    /// An opened log changed while being read.
    Changed,
}

/// A discovery diagnostic retaining its affected source path.
#[derive(Debug, Clone)]
pub struct DiscoveryIssue {
    path: PathBuf,
    kind: DiscoveryIssueKind,
}

impl DiscoveryIssue {
    pub(crate) fn new(path: &Path, kind: DiscoveryIssueKind) -> Self {
        Self {
            path: path.to_path_buf(),
            kind,
        }
    }

    /// Returns the affected source path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the typed failure.
    #[must_use]
    pub const fn kind(&self) -> &DiscoveryIssueKind {
        &self.kind
    }
}

impl fmt::Display for DiscoveryIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.path.display())?;
        match &self.kind {
            DiscoveryIssueKind::Io(error) => write!(f, "{error}"),
            DiscoveryIssueKind::Ambiguous => f.write_str("ambiguous ASCII-case path component"),
            DiscoveryIssueKind::Limit(limit) => write!(f, "discovery limit reached: {limit}"),
            DiscoveryIssueKind::MalformedConfig => {
                f.write_str("malformed Steam library configuration")
            }
            DiscoveryIssueKind::Changed => f.write_str("launcher log changed while being read"),
        }
    }
}

impl Error for DiscoveryIssue {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            DiscoveryIssueKind::Io(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

/// A usable partial value and the reasons discovery could not be completed.
#[derive(Debug, Clone)]
pub struct DiscoveryReport<T> {
    value: T,
    issues: Vec<DiscoveryIssue>,
}

impl<T> DiscoveryReport<T> {
    pub(crate) const fn new(value: T, issues: Vec<DiscoveryIssue>) -> Self {
        Self { value, issues }
    }

    /// Borrows the successfully discovered value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Returns the retained diagnostics.
    #[must_use]
    pub fn issues(&self) -> &[DiscoveryIssue] {
        &self.issues
    }

    /// Returns whether no source or work-limit failure occurred.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.issues.is_empty()
    }

    /// Consumes the report and returns its usable value.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }

    /// Consumes the report into its usable value and diagnostics.
    #[must_use]
    pub fn into_parts(self) -> (T, Vec<DiscoveryIssue>) {
        (self.value, self.issues)
    }
}
