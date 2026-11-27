// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

/// The bounded resource consumed by discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiscoveryLimit {
    /// Filesystem and file-open operations.
    Operations,
    /// Aggregate directory entries.
    Entries,
    /// Entries in one complete directory snapshot.
    DirectoryEntries,
    /// Candidate roots.
    Roots,
    /// Admitted Wine prefixes.
    Prefixes,
    /// Windows-user entries.
    Users,
    /// Launcher log sources.
    Sources,
    /// Opened Steam configuration files.
    ConfigFiles,
    /// Aggregate configuration bytes.
    ConfigBytes,
    /// Bytes in one configuration file.
    ConfigFileBytes,
    /// Active configuration object nesting.
    ConfigDepth,
    /// Aggregate configuration tokens.
    ConfigTokens,
    /// Parsed launcher facts.
    LogFacts,
    /// Aggregate retained fact payload bytes.
    FactBytes,
    /// Ordered interpretation replays across every source context.
    ReplayedFacts,
    /// Aggregate replayed payload bytes.
    ReplayedBytes,
}

impl fmt::Display for DiscoveryLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Operations => "filesystem operations",
            Self::Entries => "directory entries",
            Self::DirectoryEntries => "entries per directory",
            Self::Roots => "candidate roots",
            Self::Prefixes => "Wine prefixes",
            Self::Users => "Windows users",
            Self::Sources => "launcher sources",
            Self::ConfigFiles => "configuration files",
            Self::ConfigBytes => "aggregate configuration bytes",
            Self::ConfigFileBytes => "configuration file bytes",
            Self::ConfigDepth => "configuration nesting",
            Self::ConfigTokens => "configuration tokens",
            Self::LogFacts => "parsed launcher facts",
            Self::FactBytes => "parsed fact payload bytes",
            Self::ReplayedFacts => "launcher context replays",
            Self::ReplayedBytes => "replayed fact payload bytes",
        })
    }
}

/// Aggregate bounds for one refreshed prefix and launcher-source snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct DiscoveryLimits {
    /// The maximum filesystem and file-open operations.
    pub operations: usize,
    /// The maximum aggregate directory entries.
    pub entries: usize,
    /// The maximum entries in one complete directory snapshot.
    pub entries_per_directory: usize,
    /// The maximum candidate roots.
    pub roots: usize,
    /// The maximum admitted prefixes.
    pub prefixes: usize,
    /// The maximum examined Windows-user entries.
    pub users: usize,
    /// The maximum admitted launcher log sources.
    pub sources: usize,
    /// The maximum opened configuration files.
    pub config_files: usize,
    /// The maximum aggregate configuration bytes.
    pub config_bytes: usize,
    /// The maximum bytes in one configuration file.
    pub config_file_bytes: usize,
    /// The maximum object nesting in a configuration file.
    pub config_depth: usize,
    /// The maximum aggregate configuration tokens.
    pub config_tokens: usize,
    /// The maximum parsed facts.
    pub log_facts: usize,
    /// The maximum aggregate parsed path/version/change payload bytes.
    pub fact_bytes: usize,
    /// The maximum aggregate interpreted fact replays.
    pub replayed_facts: usize,
    /// The maximum aggregate interpreted path/version/change payload bytes.
    pub replayed_bytes: usize,
}

impl Default for DiscoveryLimits {
    fn default() -> Self {
        Self {
            operations: 16_384,
            entries: 16_384,
            entries_per_directory: 4_096,
            roots: 256,
            prefixes: 1_024,
            users: 2_048,
            sources: 1_024,
            config_files: 32,
            config_bytes: 4 << 20,
            config_file_bytes: 1 << 20,
            config_depth: 16,
            config_tokens: 65_536,
            log_facts: 1_048_576,
            fact_bytes: 32 << 20,
            replayed_facts: 1_048_576,
            replayed_bytes: 32 << 20,
        }
    }
}

#[derive(Clone, Default)]
struct Counts {
    operations: usize,
    entries: usize,
    roots: usize,
    prefixes: usize,
    users: usize,
    sources: usize,
    config_files: usize,
    config_bytes: usize,
    config_tokens: usize,
    log_facts: usize,
    fact_bytes: usize,
    replayed_facts: usize,
    replayed_bytes: usize,
}

#[derive(Clone)]
pub(crate) struct DiscoveryBudget {
    pub(crate) limits: DiscoveryLimits,
    counts: Counts,
    exhausted: bool,
    facts_exhausted: bool,
    replay_exhausted: bool,
}

impl DiscoveryBudget {
    pub(crate) fn new(limits: DiscoveryLimits) -> Self {
        Self {
            limits,
            counts: Counts::default(),
            exhausted: false,
            facts_exhausted: false,
            replay_exhausted: false,
        }
    }

    pub(crate) const fn exhausted(&self) -> bool {
        self.exhausted
    }

    pub(crate) const fn facts_exhausted(&self) -> bool {
        self.facts_exhausted
    }

    pub(crate) const fn replay_exhausted(&self) -> bool {
        self.replay_exhausted
    }

    pub(crate) const fn admit(&mut self, kind: DiscoveryLimit, count: usize) -> bool {
        let (used, limit) = match kind {
            DiscoveryLimit::Operations => (&mut self.counts.operations, self.limits.operations),
            DiscoveryLimit::Entries => (&mut self.counts.entries, self.limits.entries),
            DiscoveryLimit::Roots => (&mut self.counts.roots, self.limits.roots),
            DiscoveryLimit::Prefixes => (&mut self.counts.prefixes, self.limits.prefixes),
            DiscoveryLimit::Users => (&mut self.counts.users, self.limits.users),
            DiscoveryLimit::Sources => (&mut self.counts.sources, self.limits.sources),
            DiscoveryLimit::ConfigFiles => {
                (&mut self.counts.config_files, self.limits.config_files)
            }
            DiscoveryLimit::ConfigBytes => {
                (&mut self.counts.config_bytes, self.limits.config_bytes)
            }
            DiscoveryLimit::ConfigTokens => {
                (&mut self.counts.config_tokens, self.limits.config_tokens)
            }
            DiscoveryLimit::LogFacts => (&mut self.counts.log_facts, self.limits.log_facts),
            DiscoveryLimit::FactBytes => (&mut self.counts.fact_bytes, self.limits.fact_bytes),
            DiscoveryLimit::ReplayedFacts => {
                (&mut self.counts.replayed_facts, self.limits.replayed_facts)
            }
            DiscoveryLimit::ReplayedBytes => {
                (&mut self.counts.replayed_bytes, self.limits.replayed_bytes)
            }
            DiscoveryLimit::DirectoryEntries
            | DiscoveryLimit::ConfigFileBytes
            | DiscoveryLimit::ConfigDepth => return false,
        };
        if count > limit.saturating_sub(*used) {
            match kind {
                DiscoveryLimit::LogFacts | DiscoveryLimit::FactBytes => self.facts_exhausted = true,
                DiscoveryLimit::ReplayedFacts | DiscoveryLimit::ReplayedBytes => {
                    self.replay_exhausted = true;
                }
                _ => self.exhausted = true,
            }
            return false;
        }
        *used += count;
        true
    }
}

#[cfg(test)]
#[path = "discovery_limits_tests.rs"]
mod tests;
