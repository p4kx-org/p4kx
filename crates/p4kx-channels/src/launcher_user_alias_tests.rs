// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::detect::SearchPaths;
use crate::discovery_limits::{DiscoveryLimit, DiscoveryLimits};
use crate::discovery_report::DiscoveryIssueKind;
use crate::launcher_snapshot::LauncherSnapshot;
use crate::launcher_test_support as support;
use crate::prefix_fs::{DiscoveryPathKind, NativePrefixFileSystem, PrefixFileSystem};

use support::TestResult;

struct AliasFs<'path> {
    alias: &'path Path,
    primary: &'path Path,
}

impl PrefixFileSystem for AliasFs<'_> {
    type Entries = <NativePrefixFileSystem as PrefixFileSystem>::Entries;
    type Config = <NativePrefixFileSystem as PrefixFileSystem>::Config;

    fn kind(&self, path: &Path) -> io::Result<DiscoveryPathKind> {
        NativePrefixFileSystem.kind(path)
    }
    fn read_dir(&self, path: &Path) -> io::Result<Self::Entries> {
        NativePrefixFileSystem.read_dir(path)
    }
    fn open_config(&self, path: &Path) -> io::Result<Self::Config> {
        NativePrefixFileSystem.open_config(path)
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        path.strip_prefix(self.alias).map_or_else(
            |_| NativePrefixFileSystem.canonicalize(path),
            |tail| NativePrefixFileSystem.canonicalize(&self.primary.join(tail)),
        )
    }
}

struct Fixture {
    _temporary: tempfile::TempDir,
    prefix: PathBuf,
    primary: PathBuf,
    alias: PathBuf,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let temporary = tempfile::tempdir()?;
        let prefix = temporary.path().join("prefix");
        let a = support::launcher(&prefix, "a")?;
        let m = support::launcher(&prefix, "m")?;
        let z = support::launcher(&prefix, "z")?;
        fs::write(a.join("log.log"), support::patch("5.0.0"))?;
        fs::write(m.join("log.log"), support::patch("5.0.1"))?;
        fs::hard_link(a.join("log.log"), z.join("log.log"))?;
        let users = prefix.join("drive_c/users");
        Ok(Self {
            primary: users.join("a"),
            alias: users.join("z"),
            prefix,
            _temporary: temporary,
        })
    }

    fn snapshot(&self, limits: DiscoveryLimits) -> LauncherSnapshot {
        SearchPaths {
            wine_prefixes: vec![self.prefix.clone()],
            ..SearchPaths::default()
        }
        .launcher_snapshot_with(
            &AliasFs {
                alias: &self.alias,
                primary: &self.primary,
            },
            limits,
        )
    }

    fn remove_alias(&self) -> io::Result<()> {
        let mut folder = self.alias.join("AppData/Roaming/rsilauncher/logs");
        fs::remove_file(folder.join("log.log"))?;
        while folder.starts_with(&self.alias) {
            fs::remove_dir(&folder)?;
            if !folder.pop() {
                break;
            }
        }
        Ok(())
    }
}

#[test]
fn same_prefix_user_aliases_keep_a_c_a_order_with_one_physical_capture_per_file() -> TestResult {
    let fixture = Fixture::new()?;
    let limits = DiscoveryLimits {
        log_facts: 2,
        replayed_facts: 3,
        ..DiscoveryLimits::default()
    };
    let snapshot = fixture.snapshot(limits);
    assert_source_occurrences(&snapshot, &fixture);
    let patches = snapshot.read_patches();
    assert!(
        patches.is_complete(),
        "two capture facts and three ordered replay facts suffice"
    );
    assert_eq!(
        patches.value().get("42"),
        Some("5.0.0"),
        "the later z alias replays A after m's C"
    );
    fixture.remove_alias()?;
    let next = fixture.snapshot(limits);
    assert_eq!(
        next.occurrences.len(),
        2,
        "removing z removes precisely its interpretation occurrence"
    );
    assert_ne!(
        snapshot.stamp(),
        next.stamp(),
        "alias removal invalidates the source-aware cache key"
    );
    let patches = next.read_patches();
    assert!(
        patches.is_complete(),
        "the remaining a/m sources are complete"
    );
    assert_eq!(
        patches.value().get("42"),
        Some("5.0.1"),
        "C wins after the later A disappears"
    );
    Ok(())
}

#[test]
fn same_prefix_alias_occurrences_consume_source_and_replay_admission() -> TestResult {
    let fixture = Fixture::new()?;
    let limits = DiscoveryLimits {
        sources: 2,
        ..DiscoveryLimits::default()
    };
    let snapshot = fixture.snapshot(limits);
    assert_eq!(
        snapshot.occurrences.len(),
        2,
        "a source cut cannot retain the unadmitted z occurrence"
    );
    assert!(
        snapshot.issues().iter().any(|issue| matches!(
            issue.kind(),
            DiscoveryIssueKind::Limit(DiscoveryLimit::Sources)
        )),
        "an alias consumes a source slot"
    );
    let limits = DiscoveryLimits {
        log_facts: 2,
        replayed_facts: 2,
        ..DiscoveryLimits::default()
    };
    let patches = fixture.snapshot(limits).read_patches();
    assert!(
        !patches.is_complete(),
        "two physical facts cannot hide a third unadmitted replay"
    );
    assert!(
        patches.issues().iter().any(|issue| matches!(
            issue.kind(),
            DiscoveryIssueKind::Limit(DiscoveryLimit::ReplayedFacts)
        )),
        "replay admission remains aggregate"
    );
    assert_eq!(
        patches.value().get("42"),
        Some("5.0.1"),
        "the admitted a/m replay survives the cut"
    );
    Ok(())
}

fn assert_source_occurrences(snapshot: &LauncherSnapshot, fixture: &Fixture) {
    assert!(
        snapshot.is_complete(),
        "the synthetic directory identities are readable: {:?}",
        snapshot.issues()
    );
    assert_eq!(
        snapshot.logs.len(),
        2,
        "A and C each have one actual opened physical owner"
    );
    assert_eq!(
        snapshot.occurrences.len(),
        3,
        "all sorted a/m/z user occurrences are retained"
    );
    let folders: Vec<_> = snapshot
        .occurrences
        .iter()
        .map(|source| source.folder.clone())
        .collect();
    let expected = ["a", "m", "z"].map(|name| {
        fixture
            .prefix
            .join("drive_c/users")
            .join(name)
            .join("AppData/Roaming/rsilauncher/logs")
    });
    assert_eq!(
        folders, expected,
        "the selector retains every original user path in sorted order"
    );
}
