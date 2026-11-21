// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, EntryIssue};

use crate::export_ledger::ArchiveInventoryProblem;

/// Reads actual root-wide directory uncertainty without assigning it to an Entry identity.
#[must_use]
pub(crate) fn inventory(archive: &Archive) -> Option<ArchiveInventoryProblem> {
    problems(archive.issues())
}

fn problems(issues: &[(usize, EntryIssue)]) -> Option<ArchiveInventoryProblem> {
    issues
        .iter()
        .filter_map(|(_, issue)| {
            if !issue.is_archive_wide() {
                return None;
            }
            Some(match issue {
                EntryIssue::Recovered => ArchiveInventoryProblem::Recovered,
                EntryIssue::RecoveryLimit => ArchiveInventoryProblem::RecoveryLimit,
                _ => ArchiveInventoryProblem::Uncertain,
            })
        })
        .max()
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::EntryIssue;

    use crate::export_ledger::ArchiveInventoryProblem;

    #[test]
    fn recovery_limits_retain_the_stronger_global_uncertainty() {
        let issues = [(0, EntryIssue::Recovered), (0, EntryIssue::RecoveryLimit)];
        assert_eq!(
            super::problems(&issues),
            Some(ArchiveInventoryProblem::RecoveryLimit),
            "the explicit scan limit is not hidden by the general recovery marker"
        );
    }

    #[test]
    fn ordinary_entry_issues_do_not_fabricate_global_recovery() {
        assert_eq!(
            super::problems(&[(0, EntryIssue::DataOutsideFile)]),
            None,
            "an Entry-specific fault is not evidence that the whole directory was recovered"
        );
        assert_eq!(
            super::problems(&[]),
            None,
            "a healthy directory carries no root inventory uncertainty"
        );
    }
}
