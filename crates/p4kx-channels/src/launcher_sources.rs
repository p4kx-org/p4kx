// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::array;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use same_file::Handle;

use crate::detect::SearchPaths;
use crate::discovery_context::DiscoveryContext;
use crate::discovery_limits::{DiscoveryLimit, DiscoveryLimits};
use crate::launcher_log::{LAUNCHER_LOG_FILES, launcher_logs_folder};
use crate::launcher_snapshot::{LauncherOccurrence, LauncherSnapshot};
use crate::launcher_stamp::{LauncherStamp, LibraryStamp, LogFileStamp, SourceStamp};
use crate::opened_log::{self, OpenedLog};
use crate::prefix_case;
use crate::prefix_fs::{NativePrefixFileSystem, PrefixFileSystem};
use crate::wine_context::WineContext;

pub(crate) fn build<F: PrefixFileSystem>(
    paths: SearchPaths,
    context: DiscoveryContext<'_, F>,
) -> LauncherSnapshot {
    let mut sources = Sources::new(paths, context);
    sources.default_libraries();
    if let Some(folder) = sources
        .paths
        .launcher
        .as_ref()
        .map(|path| launcher_logs_folder(path))
    {
        sources.source(folder, None);
    }
    let prefixes = std::mem::take(&mut sources.paths.wine_prefixes);
    for prefix in &prefixes {
        if sources.context.budget.exhausted() {
            break;
        }
        sources.prefix(prefix);
    }
    sources.paths.wine_prefixes = prefixes;
    sources.finish()
}

pub(crate) fn log_folder(folder: &Path, limits: DiscoveryLimits) -> LauncherSnapshot {
    let fs = NativePrefixFileSystem;
    let context = DiscoveryContext::new(&fs, limits);
    let mut sources = Sources::new(SearchPaths::default(), context);
    sources.stamp.roots.push(folder.to_path_buf());
    sources.source(folder.to_path_buf(), None);
    sources.finish()
}

struct Sources<'fs, F: PrefixFileSystem> {
    paths: SearchPaths,
    context: DiscoveryContext<'fs, F>,
    stamp: LauncherStamp,
    occurrences: Vec<LauncherOccurrence>,
    logs: Vec<OpenedLog>,
    identities: HashMap<Arc<Handle>, usize>,
}

impl<'fs, F: PrefixFileSystem> Sources<'fs, F> {
    fn new(paths: SearchPaths, context: DiscoveryContext<'fs, F>) -> Self {
        let roots = paths
            .launcher
            .iter()
            .chain(&paths.wine_prefixes)
            .cloned()
            .collect();
        Self {
            paths,
            context,
            stamp: LauncherStamp {
                roots,
                libraries: Vec::new(),
                sources: Vec::new(),
            },
            occurrences: Vec::new(),
            logs: Vec::new(),
            identities: HashMap::new(),
        }
    }

    fn finish(self) -> LauncherSnapshot {
        LauncherSnapshot {
            paths: self.paths,
            stamp: self.stamp,
            occurrences: self.occurrences,
            logs: self.logs,
            budget: self.context.budget,
            issues: self.context.issues,
        }
    }

    fn default_libraries(&mut self) {
        if let Some(path) = self.paths.default_library.clone() {
            let canonical = self.context.canonical(&path);
            self.stamp.libraries.push(LibraryStamp { path, canonical });
        }
        for prefix in &self.paths.wine_prefixes {
            if self.context.budget.exhausted() {
                break;
            }
            let path = prefix_case::descend(
                &mut self.context,
                prefix,
                &[
                    "drive_c",
                    "Program Files",
                    "Roberts Space Industries",
                    "StarCitizen",
                ],
            )
            .unwrap_or_else(|| crate::wine::game_library(prefix));
            let canonical = self.context.canonical(&path);
            if !self.stamp.libraries.iter().any(|known| known.path == path) {
                self.stamp.libraries.push(LibraryStamp { path, canonical });
            }
        }
    }

    fn prefix(&mut self, prefix: &Path) {
        let interpretation = Arc::new(WineContext::discover(&mut self.context, prefix));
        let drive = interpretation.c_drive();
        let Some(users) = prefix_case::descend(&mut self.context, drive, &["users"]) else {
            return;
        };
        let Some(names) = self.context.entries(&users) else {
            return;
        };
        for name in names {
            if !self.context.admit(&users, DiscoveryLimit::Users, 1) {
                return;
            }
            let Some(user) = prefix_case::component(&mut self.context, &users, &name) else {
                continue;
            };
            let Some(logs) = prefix_case::descend(
                &mut self.context,
                &user,
                &["AppData", "Roaming", "rsilauncher", "logs"],
            ) else {
                continue;
            };
            self.source(logs, Some(Arc::clone(&interpretation)));
        }
    }

    fn source(&mut self, folder: PathBuf, prefix: Option<Arc<WineContext>>) {
        if !self.context.admit(&folder, DiscoveryLimit::Sources, 1) {
            return;
        }
        let mut states = array::from_fn(|_| LogFileStamp::Missing);
        let mut files = [None; LAUNCHER_LOG_FILES.len()];
        for ((state, slot), name) in states.iter_mut().zip(&mut files).zip(LAUNCHER_LOG_FILES) {
            (*state, *slot) = opened_log::open(
                &mut self.context,
                &folder.join(name),
                &mut self.logs,
                &mut self.identities,
                self.occurrences.len(),
                name,
            );
        }
        self.stamp.sources.push(SourceStamp {
            folder: folder.clone(),
            prefix: prefix.clone(),
            files: states,
        });
        self.occurrences.push(LauncherOccurrence {
            folder,
            prefix,
            files,
        });
    }
}

#[cfg(test)]
#[path = "launcher_user_alias_tests.rs"]
mod tests;
