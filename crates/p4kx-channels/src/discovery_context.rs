// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::discovery_limits::{DiscoveryBudget, DiscoveryLimit, DiscoveryLimits};
use crate::discovery_report::{DiscoveryIssue, DiscoveryIssueKind};
use crate::prefix_fs::{DiscoveryPathKind, PrefixFileSystem};

pub(crate) struct DiscoveryContext<'fs, F: PrefixFileSystem> {
    pub(crate) fs: &'fs F,
    pub(crate) budget: DiscoveryBudget,
    pub(crate) issues: Vec<DiscoveryIssue>,
}

impl<'fs, F: PrefixFileSystem> DiscoveryContext<'fs, F> {
    pub(crate) fn new(fs: &'fs F, limits: DiscoveryLimits) -> Self {
        Self {
            fs,
            budget: DiscoveryBudget::new(limits),
            issues: Vec::new(),
        }
    }

    pub(crate) fn issue(&mut self, path: &Path, kind: DiscoveryIssueKind) {
        self.issues.push(DiscoveryIssue::new(path, kind));
    }

    pub(crate) fn io(&mut self, path: &Path, error: io::Error) {
        self.issue(path, DiscoveryIssueKind::Io(Arc::new(error)));
    }

    pub(crate) fn absent_or_error(&mut self, path: &Path, error: io::Error) {
        if error.kind() != io::ErrorKind::NotFound {
            self.io(path, error);
        }
    }

    pub(crate) fn admit(&mut self, path: &Path, kind: DiscoveryLimit, count: usize) -> bool {
        let exhausted = match kind {
            DiscoveryLimit::LogFacts | DiscoveryLimit::FactBytes => self.budget.facts_exhausted(),
            DiscoveryLimit::ReplayedFacts | DiscoveryLimit::ReplayedBytes => {
                self.budget.replay_exhausted()
            }
            _ => self.budget.exhausted(),
        };
        if exhausted {
            return false;
        }
        if self.budget.admit(kind, count) {
            return true;
        }
        self.issue(path, DiscoveryIssueKind::Limit(kind));
        false
    }

    pub(crate) fn fact(&mut self, path: &Path, bytes: usize) -> bool {
        self.admit(path, DiscoveryLimit::LogFacts, 1)
            && self.admit(path, DiscoveryLimit::FactBytes, bytes)
    }

    pub(crate) fn replay(&mut self, path: &Path, bytes: usize) -> bool {
        self.admit(path, DiscoveryLimit::ReplayedFacts, 1)
            && self.admit(path, DiscoveryLimit::ReplayedBytes, bytes)
    }

    pub(crate) fn kind(&mut self, path: &Path) -> Option<DiscoveryPathKind> {
        if !self.admit(path, DiscoveryLimit::Operations, 1) {
            return None;
        }
        match self.fs.kind(path) {
            Ok(kind) => Some(kind),
            Err(error) => {
                self.absent_or_error(path, error);
                None
            }
        }
    }

    pub(crate) fn canonical(&mut self, path: &Path) -> Option<PathBuf> {
        if !self.admit(path, DiscoveryLimit::Operations, 1) {
            return None;
        }
        match self.fs.canonicalize(path) {
            Ok(path) => Some(path),
            Err(error) => {
                self.absent_or_error(path, error);
                None
            }
        }
    }

    pub(crate) fn entries(&mut self, path: &Path) -> Option<Vec<OsString>> {
        if !self.admit(path, DiscoveryLimit::Operations, 1) {
            return None;
        }
        let entries = match self.fs.read_dir(path) {
            Ok(entries) => entries,
            Err(error) => {
                self.absent_or_error(path, error);
                return None;
            }
        };
        let mut names = Vec::new();
        for entry in entries {
            if !self.admit(path, DiscoveryLimit::Entries, 1) {
                return None;
            }
            if names.len() >= self.budget.limits.entries_per_directory {
                self.issue(
                    path,
                    DiscoveryIssueKind::Limit(DiscoveryLimit::DirectoryEntries),
                );
                return None;
            }
            match entry {
                Ok(name) => names.push(name),
                Err(error) => {
                    self.io(path, error);
                    return None;
                }
            }
        }
        names.sort();
        Some(names)
    }
}
