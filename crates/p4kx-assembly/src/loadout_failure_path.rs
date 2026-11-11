// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::{Display, Formatter, Result as FmtResult};
use std::ops::Deref;
use std::sync::Arc;

use crate::{LoadoutFileObservation, LoadoutPath, LoadoutRequest};

/// The original path of an absent file and the actual attempts for this use.
#[derive(Debug, Clone)]
pub struct LoadoutFailurePath {
    path: LoadoutPath,
    evidence: Option<Arc<FailureEvidence>>,
}

#[derive(Debug, Clone)]
struct FailureEvidence {
    attempts: Arc<[LoadoutFileObservation]>,
    includes: Option<Arc<[LoadoutPath]>>,
    caller: Option<Arc<LoadoutRequest>>,
}

impl LoadoutFailurePath {
    /// Creates a missing-path descriptor without an original read claim.
    #[must_use]
    pub fn unbound(path: impl Into<Arc<str>>) -> Self {
        Self::from_attempts(LoadoutPath::unbound(path), None)
    }

    /// Borrows the original path of this missing file.
    #[must_use]
    pub const fn path(&self) -> &LoadoutPath {
        &self.path
    }

    /// Borrows the original attempts that produced this missing outcome.
    #[must_use]
    pub fn attempts(&self) -> &[LoadoutFileObservation] {
        self.evidence
            .as_ref()
            .map_or(&[], |evidence| evidence.attempts.as_ref())
    }

    /// Borrows this occurrence's actual Include path from outermost to innermost.
    #[must_use]
    pub fn includes(&self) -> &[LoadoutPath] {
        self.evidence
            .as_ref()
            .and_then(|evidence| evidence.includes.as_deref())
            .unwrap_or_default()
    }

    /// Borrows the caller that received this result from the resolver.
    #[must_use]
    pub fn caller(&self) -> Option<&LoadoutRequest> {
        self.evidence
            .as_ref()
            .and_then(|evidence| evidence.caller.as_deref())
    }

    pub(crate) fn from_attempts(
        path: LoadoutPath,
        attempts: Option<Arc<[LoadoutFileObservation]>>,
    ) -> Self {
        Self {
            path,
            evidence: attempts.map(|attempts| {
                Arc::new(FailureEvidence {
                    attempts,
                    includes: None,
                    caller: None,
                })
            }),
        }
    }

    pub(crate) fn selected_by(&mut self, includes: &[LoadoutPath]) {
        if let Some(evidence) = &mut self.evidence {
            Arc::make_mut(evidence).includes = Some(Arc::from(includes));
        }
    }

    pub(crate) fn used_by(&mut self, caller: &Arc<LoadoutRequest>) {
        if let Some(evidence) = &mut self.evidence {
            Arc::make_mut(evidence).caller = Some(Arc::clone(caller));
        }
    }
}

impl PartialEq for LoadoutFailurePath {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for LoadoutFailurePath {}

impl Deref for LoadoutFailurePath {
    type Target = LoadoutPath;

    fn deref(&self) -> &Self::Target {
        self.path()
    }
}

impl Display for LoadoutFailurePath {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&self.path, formatter)
    }
}
