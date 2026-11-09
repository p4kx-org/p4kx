// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;
use std::sync::Arc;

use p4kx_channels::{Channel, DiscoveryIssue, SearchPaths, SearchRecipe};
use p4kx_settings::ManualChannels;

use crate::channel_refresh::{ChannelSearch, Measurement};

/// The Channels the picker offers: detected in the standard places and the manual entries.
#[derive(Debug, Clone, Default)]
pub struct ChannelList {
    recipe: Arc<SearchRecipe>,
    manual: ManualChannels,
    added: ManualChannels,
    detected: Vec<Channel>,
    discovery_issue_count: usize,
    issues: Vec<DiscoveryIssue>,
}

impl ChannelList {
    /// Detects the Channels in `base` and in `manual`, which replaces the manual entries of
    /// `base`.
    #[must_use]
    pub fn new(base: SearchPaths, manual: ManualChannels) -> Self {
        Self::from_recipe(SearchRecipe::explicit(base), manual)
    }

    /// Captures platform inputs and reports incomplete automatic discovery in the picker.
    #[must_use]
    pub fn standard(manual: ManualChannels) -> Self {
        Self::from_recipe(SearchRecipe::standard(), manual)
    }

    /// Detects Channels with an owned recipe and the supplied manual entries.
    #[must_use]
    pub fn from_recipe(recipe: SearchRecipe, manual: ManualChannels) -> Self {
        let mut list = Self {
            recipe: Arc::new(recipe.with_manual(Vec::new(), Vec::new())),
            manual,
            added: ManualChannels::default(),
            detected: Vec::new(),
            discovery_issue_count: 0,
            issues: Vec::new(),
        };
        list.detect();
        list
    }

    /// Retains installation-protection diagnostics from startup alongside current discovery.
    #[must_use]
    pub fn with_startup_issues(mut self, issues: Vec<DiscoveryIssue>) -> Self {
        self.record_protection_issues(issues);
        self
    }

    pub(crate) fn record_protection_issues(&mut self, issues: Vec<DiscoveryIssue>) {
        self.issues.truncate(self.discovery_issue_count);
        self.issues.extend(issues);
    }

    /// Returns current discovery failures and the latest installation-protection report.
    #[must_use]
    pub fn discovery_issues(&self) -> &[DiscoveryIssue] {
        &self.issues
    }

    /// Returns the detected Channels in the order of [`p4kx_channels::detect`].
    #[must_use]
    pub fn detected(&self) -> &[Channel] {
        &self.detected
    }

    /// Returns the manual entries.
    #[must_use]
    pub const fn manual(&self) -> &ManualChannels {
        &self.manual
    }

    /// Returns the manual entries added through [`Self::add`].
    #[must_use]
    pub const fn added(&self) -> &ManualChannels {
        &self.added
    }

    /// Registers `path` as a manual entry and returns whether it was not listed yet.
    /// [`crate::Model`] refreshes automatic discovery off the UI thread.
    pub fn add(&mut self, path: PathBuf) -> bool {
        let path = std::path::absolute(&path).unwrap_or(path);
        if path.is_file() {
            self.added.add_archive(path.clone());
            self.manual.add_archive(path)
        } else {
            self.added.add_folder(path.clone());
            self.manual.add_folder(path)
        }
    }

    pub(crate) fn search(&self) -> ChannelSearch {
        ChannelSearch {
            recipe: Arc::clone(&self.recipe),
            manual: self.manual.clone(),
        }
    }

    fn detect(&mut self) {
        self.adopt(self.search().measure());
    }

    pub(crate) fn adopt(&mut self, measured: Measurement) {
        let (found, issues) = measured.into_parts();
        let (detected, patches) = found.into_parts();
        self.detected = detected;
        let protection = self.issues.split_off(self.discovery_issue_count);
        self.issues = issues;
        self.discovery_issue_count = self.issues.len();
        self.issues.extend(protection);
        crate::public_patches::publish(patches);
    }
}
