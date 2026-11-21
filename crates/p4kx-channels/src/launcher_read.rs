// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek};
use std::ops::ControlFlow;
use std::path::Path;

use crate::PublicPatches;
use crate::discovery_context::DiscoveryContext;
use crate::discovery_limits::DiscoveryLimit;
use crate::discovery_report::{DiscoveryIssueKind, DiscoveryReport};
use crate::launcher_log::{self, LauncherLog, Wanted};
use crate::launcher_snapshot::LauncherOccurrence;
use crate::launcher_snapshot::LauncherSnapshot;
use crate::log_lines::{self, PATCH_LINE_BYTES, Part, ReadEnd};
use crate::opened_log::OpenedLog;
use crate::ordered_libraries::OrderedLibraries;
use crate::prefix_case;
use crate::prefix_fs::NativePrefixFileSystem;

pub(crate) fn scan(snapshot: &LauncherSnapshot, wanted: Wanted) -> DiscoveryReport<LauncherLog> {
    let fs = NativePrefixFileSystem;
    let mut context = DiscoveryContext::new(&fs, snapshot.budget.limits);
    context.budget = snapshot.budget.clone();
    context.issues.clone_from(&snapshot.issues);
    let mut parsed = Vec::with_capacity(snapshot.logs.len());
    for log in &snapshot.logs {
        if context.budget.exhausted() || context.budget.facts_exhausted() {
            break;
        }
        let path = snapshot
            .occurrences
            .get(log.source)
            .map(|source| source.folder.join(log.name));
        let Some(path) = path else {
            context.io(
                Path::new(log.name),
                io::Error::other("missing launcher source occurrence"),
            );
            break;
        };
        parsed.push(parse(&mut context, &path, log, wanted));
    }
    let mut output = LauncherLog::default();
    let mut found_libraries = OrderedLibraries::default();
    for occurrence in &snapshot.occurrences {
        if wanted.libraries {
            for index in occurrence.files.iter().flatten() {
                if let Some(log) = parsed.get(*index) {
                    libraries(&mut context, occurrence, log, &mut found_libraries);
                }
            }
        }
        if wanted.patches {
            for index in occurrence.files.iter().rev().flatten() {
                let Some(log) = parsed.get(*index) else {
                    continue;
                };
                output.patches.absorb_admitted(&log.patches, |bytes| {
                    context.replay(&occurrence.folder, bytes)
                });
            }
        }
        if context.budget.replay_exhausted() {
            break;
        }
    }
    output.libraries = found_libraries.finish();
    DiscoveryReport::new(output, context.issues)
}

struct ParsedInput {
    log: LauncherLog,
    complete: bool,
}

fn parse(
    context: &mut DiscoveryContext<'_, NativePrefixFileSystem>,
    path: &Path,
    log: &OpenedLog,
    wanted: Wanted,
) -> LauncherLog {
    if !context.admit(path, DiscoveryLimit::Operations, 1) {
        return LauncherLog::default();
    }
    let parsed =
        match log.with_file(|file| Ok(parse_input(context, path, file, log.length, wanted))) {
            Ok(parsed) => parsed,
            Err(error) => {
                context.io(path, error);
                return LauncherLog::default();
            }
        };
    if parsed.complete {
        match log.identity.as_file().metadata() {
            Ok(metadata)
                if metadata.len() == log.length
                    && metadata
                        .modified()
                        .is_ok_and(|modified| modified == log.modified) => {}
            Ok(_) => context.issue(path, DiscoveryIssueKind::Changed),
            Err(error) => context.io(path, error),
        }
    }
    parsed.log
}

fn parse_input(
    context: &mut DiscoveryContext<'_, NativePrefixFileSystem>,
    path: &Path,
    input: impl Read + Seek,
    length: u64,
    wanted: Wanted,
) -> ParsedInput {
    let mut facts = Facts::default();
    let part = if wanted.libraries {
        Part::Whole
    } else {
        Part::Tail
    };
    let read = log_lines::for_each_opened(input, length, part, |in_tail, bytes| {
        facts.line(context, path, wanted, in_tail, bytes)
    });
    let complete = match read {
        Ok(ReadEnd::Complete) => true,
        Ok(ReadEnd::Stopped) => false,
        Err(error) => {
            context.io(path, error);
            false
        }
    };
    facts.patches.trim();
    ParsedInput {
        log: LauncherLog {
            libraries: facts.libraries.finish(),
            patches: facts.patches,
        },
        complete,
    }
}

#[derive(Default)]
struct Facts {
    libraries: OrderedLibraries,
    patches: PublicPatches,
}

impl Facts {
    fn line(
        &mut self,
        context: &mut DiscoveryContext<'_, NativePrefixFileSystem>,
        path: &Path,
        wanted: Wanted,
        in_tail: bool,
        bytes: &[u8],
    ) -> ControlFlow<()> {
        let line = String::from_utf8_lossy(bytes);
        if wanted.libraries
            && let Some(folder) = launcher_log::library_folder(&line)
            && !self.libraries.contains(&folder)
        {
            if !context.fact(path, folder.as_os_str().as_encoded_bytes().len()) {
                return ControlFlow::Break(());
            }
            self.libraries.insert(folder);
        }
        if wanted.patches
            && in_tail
            && bytes.len() <= PATCH_LINE_BYTES
            && !self
                .patches
                .note_line_admitted(&line, |bytes| context.fact(path, bytes))
        {
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }
}

fn libraries(
    context: &mut DiscoveryContext<'_, NativePrefixFileSystem>,
    occurrence: &LauncherOccurrence,
    log: &LauncherLog,
    output: &mut OrderedLibraries,
) {
    for library in &log.libraries {
        if !context.admit(&occurrence.folder, DiscoveryLimit::ReplayedFacts, 1) {
            return;
        }
        let path = occurrence.prefix.as_ref().map_or_else(
            || library.is_absolute().then(|| library.clone()),
            |prefix| prefix.host_path(library),
        );
        if let Some(path) = path {
            let resolved = if occurrence.prefix.is_some() && !context.budget.exhausted() {
                prefix_case::absolute(context, &path).unwrap_or(path)
            } else {
                path
            };
            if !output.contains(&resolved) {
                if !context.admit(
                    &occurrence.folder,
                    DiscoveryLimit::ReplayedBytes,
                    resolved.as_os_str().as_encoded_bytes().len(),
                ) {
                    return;
                }
                output.insert(resolved);
            }
        }
    }
}

#[cfg(test)]
#[path = "launcher_read_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "launcher_budget_tests.rs"]
mod budget_tests;
