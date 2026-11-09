// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_channels::DiscoveryIssue;

pub(crate) struct ProtectedResult<T, E> {
    pub(crate) result: Result<T, E>,
    pub(crate) issues: Vec<DiscoveryIssue>,
}

impl<T, E> ProtectedResult<T, E> {
    pub(crate) fn read(path: &Path, read: impl FnOnce() -> Result<T, E>) -> Self {
        let issues = p4kx_extraction::protect_archive_folder_report(path);
        Self::measured(issues, read)
    }

    fn measured(issues: Vec<DiscoveryIssue>, read: impl FnOnce() -> Result<T, E>) -> Self {
        Self {
            result: read(),
            issues,
        }
    }

    pub(crate) fn zip(self, other: Self) -> ProtectedResult<(T, T), E> {
        let mut issues = self.issues;
        issues.extend(other.issues);
        let result = self
            .result
            .and_then(|left| other.result.map(|right| (left, right)));
        ProtectedResult { result, issues }
    }
}

#[cfg(test)]
#[path = "protected_open_tests.rs"]
mod tests;
