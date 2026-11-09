// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use p4kx_archive::Build;

use crate::discovery_limits::DiscoveryLimits;
use crate::discovery_report::DiscoveryReport;
use crate::launcher_log::{self, Wanted};
use crate::launcher_snapshot::LauncherSnapshot;
use crate::search_recipe::SearchRecipe;

const MARKER: &str = "(SC ";
const MAX_PATCHES: usize = 4096;
const MAX_VERSION_LEN: usize = 32;
const MAX_CHANGE_LEN: usize = 20;

/// The public patch versions the RSI Launcher logged, by Perforce change number.
///
/// Read once from the launcher's log files and handed to whoever needs a [`Build`]'s public
/// patch; an empty value gives no Build a public patch.
#[derive(Debug, Clone, Default)]
pub struct PublicPatches {
    by_change: HashMap<String, (String, u64)>,
    next: u64,
}

impl PartialEq for PublicPatches {
    fn eq(&self, other: &Self) -> bool {
        self.by_change.len() == other.by_change.len()
            && self.by_change.iter().all(|(change, (version, _))| {
                other.get(change).is_some_and(|known| known == version)
            })
    }
}

impl Eq for PublicPatches {}

impl PublicPatches {
    /// Reads standard native and Wine sources, or only `P4KX_LAUNCHER_LOGS` when set.
    ///
    /// Only `log.log` and `log.old.log` are read; a missing or unreadable file is skipped.
    #[must_use]
    pub fn standard() -> Self {
        Self::standard_report().into_value()
    }

    /// Reads standard public-patch sources with explicit discovery diagnostics.
    #[must_use]
    pub fn standard_report() -> DiscoveryReport<Self> {
        let folder = std::env::var_os("P4KX_LAUNCHER_LOGS").map(PathBuf::from);
        standard_sources(folder.as_deref(), SearchRecipe::standard)
    }

    /// Reads the snapshot's exact opened log sources in their original occurrence order.
    #[must_use]
    pub fn read_sources(snapshot: &LauncherSnapshot) -> DiscoveryReport<Self> {
        snapshot.read_patches()
    }

    /// Reads the public patches from the log files in `log_folder`.
    ///
    /// A line of the form `(SC <CHANNEL> <version>-<tag>.<change>)` names the version of that
    /// change; the newest line wins, and at most 4096 changes are kept, the most recent ones. Only
    /// the last 16 MiB of each file and lines up to 64 KiB count.
    #[must_use]
    pub fn read_from(log_folder: &Path) -> Self {
        launcher_log::scan(log_folder, Wanted::PATCHES).patches
    }

    /// Returns the public patch version, e.g. `4.7.2`, of the Build made from the change
    /// `change_number`, or `None` when no log line names that change.
    #[must_use]
    pub fn get(&self, change_number: &str) -> Option<&str> {
        self.by_change
            .get(change_number)
            .map(|(version, _)| version.as_str())
    }

    /// Returns `build` with the public patch of its change number, or without one when the log
    /// has no line for it.
    #[must_use]
    pub fn attach(&self, build: &Build) -> Build {
        build
            .clone()
            .with_public_patch(self.get(build.change_number()).map(str::to_owned))
    }

    /// Returns the public patch of `build`, without cloning the Build.
    #[must_use]
    pub fn of(&self, build: &Build) -> Option<&str> {
        self.get(build.change_number())
    }

    pub(crate) fn note_line_admitted(
        &mut self,
        line: &str,
        mut admit: impl FnMut(usize) -> bool,
    ) -> bool {
        let mut rest = line;
        while let Some((_, after)) = rest.split_once(MARKER) {
            let Some((label, tail)) = after.split_once(')') else {
                break;
            };
            if let Some((version, change)) = patch_of(label) {
                if !admit(version.len() + change.len()) {
                    return false;
                }
                self.record(change, version);
            }
            rest = tail;
        }
        true
    }

    pub(crate) fn record(&mut self, change: &str, version: &str) {
        self.next += 1;
        let stamp = self.next;
        if let Some((known, seen)) = self.by_change.get_mut(change) {
            version.clone_into(known);
            *seen = stamp;
        } else {
            self.by_change
                .insert(change.to_owned(), (version.to_owned(), stamp));
            if self.by_change.len() >= 2 * MAX_PATCHES {
                self.trim();
            }
        }
    }

    pub(crate) fn absorb_admitted(
        &mut self,
        newer: &Self,
        mut admit: impl FnMut(usize) -> bool,
    ) -> bool {
        let mut items: Vec<_> = newer
            .by_change
            .iter()
            .map(|(change, (version, stamp))| (*stamp, change.as_str(), version.as_str()))
            .collect();
        items.sort_unstable();
        for (_, change, version) in items {
            if !admit(version.len() + change.len()) {
                return false;
            }
            self.record(change, version);
        }
        self.trim();
        true
    }

    /// Keeps the [`MAX_PATCHES`] changes whose newest line is the most recent.
    pub(crate) fn trim(&mut self) {
        let excess = self.by_change.len().saturating_sub(MAX_PATCHES);
        if excess == 0 {
            return;
        }
        let mut stamps: Vec<u64> = self.by_change.values().map(|(_, stamp)| *stamp).collect();
        stamps.sort_unstable();
        if let Some(&cutoff) = stamps.get(excess) {
            self.by_change.retain(|_, (_, stamp)| *stamp >= cutoff);
        }
    }
}

fn standard_sources(
    override_folder: Option<&Path>,
    recipe: impl FnOnce() -> SearchRecipe,
) -> DiscoveryReport<PublicPatches> {
    if let Some(folder) = override_folder {
        return LauncherSnapshot::log_folder(folder, DiscoveryLimits::default()).read_patches();
    }
    recipe().snapshot().read_patches()
}

#[cfg(test)]
#[path = "standard_source_tests.rs"]
mod tests;

/// Returns the version and the change of a label `<CHANNEL> <version>-<tag>.<change>`, e.g.
/// `4.7.2` and `11674325` for `LIVE 4.7.2-live.11674325`.
fn patch_of(label: &str) -> Option<(&str, &str)> {
    let (_channel, build) = label.split_once(' ')?;
    let (version, tail) = build.split_once('-')?;
    let (_tag, change) = tail.rsplit_once('.')?;
    (is_version(version) && is_change(change)).then_some((version, change))
}

fn is_version(version: &str) -> bool {
    version.len() <= MAX_VERSION_LEN
        && version
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_change(change: &str) -> bool {
    (1..=MAX_CHANGE_LEN).contains(&change.len()) && change.bytes().all(|byte| byte.is_ascii_digit())
}
