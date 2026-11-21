// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File, FileTimes};
use std::path::{Path, PathBuf};

use p4kx_channels::{LauncherSnapshot, SearchRecipe};

use crate::Installation;

use super::LibraryProtectionCache;
use super::fixture::{self, Fixture, TestResult};

struct Rebinding {
    fixture: Fixture,
    recipe: SearchRecipe,
    log: PathBuf,
    alias: PathBuf,
    a: PathBuf,
    b: PathBuf,
    unused: PathBuf,
}

impl Rebinding {
    fn new() -> TestResult<Self> {
        let fixture = Fixture::new()?;
        let prefix = fixture.prefix("prefix with spaces")?;
        let a = Fixture::library(&prefix, "A")?;
        let b = Fixture::library(&prefix, "B")?;
        let unused = Fixture::library(&prefix, "Unused")?;
        let alias = prefix.join("drive_c/Games/Linked");
        fixture::link_directory(&alias, &a)?;
        let log = Fixture::log(&prefix, "Pilot")?;
        fixture::libraries(&log, &["Linked"])?;
        let recipe = Fixture::recipe(vec![prefix]);
        Ok(Self {
            fixture,
            recipe,
            log,
            alias,
            a,
            b,
            unused,
        })
    }

    fn retarget(&self, target: &Path) -> TestResult {
        fixture::remove_directory_link(&self.alias)?;
        fixture::link_directory(&self.alias, target)
    }

    fn snapshot(&self) -> LauncherSnapshot {
        self.recipe.snapshot()
    }
}

fn refresh(
    cache: &mut LibraryProtectionCache,
    snapshot: &LauncherSnapshot,
    installation: &mut Installation,
) -> TestResult {
    let issues = cache.refresh(snapshot, installation);
    if !issues.is_empty() {
        return Err(format!("expected complete synthetic library discovery: {issues:?}").into());
    }
    Ok(())
}

#[test]
fn an_unchanged_log_cache_rebinds_the_library_root_and_retains_both_targets_after_restore()
-> TestResult {
    let binding = Rebinding::new()?;
    let mut cache = LibraryProtectionCache::new();
    let mut installation = Installation::new();
    let original = binding.snapshot();
    refresh(&mut cache, &original, &mut installation)?;
    assert!(
        installation.check(&binding.a.join("LIVE/new.bin")).is_err(),
        "initial A is protected"
    );
    assert!(
        installation.check(&binding.b.join("PTU/new.bin")).is_ok(),
        "unlogged B starts writable"
    );
    binding.retarget(&binding.b)?;
    fs::create_dir_all(binding.b.join("PTU/Bin64"))?;
    let changed = binding.snapshot();
    assert_eq!(
        original.stamp(),
        changed.stamp(),
        "only the logged-library binding changed"
    );
    refresh(&mut cache, &changed, &mut installation)?;
    assert!(
        installation.check(&binding.b.join("PTU/new.bin")).is_err(),
        "a cache hit protects newly created Channels below the rebound library root"
    );
    assert!(
        installation
            .check(&binding.alias.join("PTU/new.bin"))
            .is_err(),
        "the current logical library namespace is protected too"
    );
    assert!(
        installation.check(&binding.a.join("LIVE/new.bin")).is_err(),
        "old A remains protected"
    );
    binding.retarget(&binding.a)?;
    let restored = binding.snapshot();
    assert_eq!(
        original.stamp(),
        restored.stamp(),
        "restoring the library alias leaves log identity unchanged"
    );
    refresh(&mut cache, &restored, &mut installation)?;
    assert!(
        installation.check(&binding.a.join("LIVE/new.bin")).is_err(),
        "restored A stays protected"
    );
    assert!(
        installation.check(&binding.b.join("PTU/new.bin")).is_err(),
        "observed B remains protected"
    );
    assert!(
        installation
            .check(&binding.fixture.root().join("export/new.bin"))
            .is_ok(),
        "an unrelated export sibling remains writable"
    );
    Ok(())
}

#[test]
fn rebinding_uses_the_saved_library_paths_without_a_second_log_parser() -> TestResult {
    let binding = Rebinding::new()?;
    let mut cache = LibraryProtectionCache::new();
    let mut installation = Installation::new();
    let original = binding.snapshot();
    refresh(&mut cache, &original, &mut installation)?;
    let before = fs::metadata(&binding.log)?;
    let modified = before.modified()?;
    fixture::libraries(&binding.log, &["Unused"])?;
    File::options()
        .write(true)
        .open(&binding.log)?
        .set_times(FileTimes::new().set_modified(modified))?;
    assert_eq!(
        fs::metadata(&binding.log)?.len(),
        before.len(),
        "the parser decoy preserves log length"
    );
    assert_eq!(
        fs::metadata(&binding.log)?.modified()?,
        modified,
        "the parser decoy preserves log time"
    );
    binding.retarget(&binding.b)?;
    fs::create_dir_all(binding.b.join("PTU/Bin64"))?;
    let rebound = binding.snapshot();
    assert_eq!(
        original.stamp(),
        rebound.stamp(),
        "the complete source identity remains an actual cache hit"
    );
    refresh(&mut cache, &rebound, &mut installation)?;
    assert!(
        installation.check(&binding.b.join("PTU/new.bin")).is_err(),
        "the stored logical Linked path is resolved to its current target"
    );
    assert!(
        installation
            .check(&binding.unused.join("LIVE/new.bin"))
            .is_ok(),
        "an unnecessary second parser would consume the same-metadata Unused decoy"
    );
    assert!(
        installation.check(&binding.a.join("LIVE/new.bin")).is_err(),
        "the old target remains protected"
    );
    assert!(
        installation
            .check(&binding.fixture.root().join("export/new.bin"))
            .is_ok(),
        "rebinding does not extend protection to unrelated output"
    );
    Ok(())
}

#[test]
fn the_measured_library_keeps_its_lexical_alias_for_future_target_resolution() -> TestResult {
    let binding = Rebinding::new()?;
    let report = binding.snapshot().read_libraries();
    assert!(
        report.is_complete(),
        "the actual source mapping is complete: {:?}",
        report.issues()
    );
    assert!(
        report.value().contains(&binding.alias),
        "the common mapper returns the logical library path rather than freezing its target"
    );
    assert!(
        !report.value().contains(&binding.a),
        "the logged alias is not replaced with only its initial canonical target"
    );
    assert_eq!(
        fs::canonicalize(&binding.alias)?,
        fs::canonicalize(&binding.a)?,
        "the synthetic alias really reaches A while its logical path remains available"
    );
    Ok(())
}

#[test]
fn an_unchanged_library_binding_keeps_protection_without_guessing_new_targets() -> TestResult {
    let binding = Rebinding::new()?;
    let mut cache = LibraryProtectionCache::new();
    let mut installation = Installation::new();
    let first = binding.snapshot();
    refresh(&mut cache, &first, &mut installation)?;
    let unchanged = binding.snapshot();
    assert_eq!(
        first.stamp(),
        unchanged.stamp(),
        "the healthy control is a complete unchanged cache hit"
    );
    refresh(&mut cache, &unchanged, &mut installation)?;
    assert!(
        installation.check(&binding.a.join("LIVE/new.bin")).is_err(),
        "A remains protected"
    );
    assert!(
        installation.check(&binding.b.join("PTU/new.bin")).is_ok(),
        "unobserved B stays writable"
    );
    assert!(
        installation
            .check(&binding.fixture.root().join("export/new.bin"))
            .is_ok(),
        "the unchanged control leaves a real export sibling writable"
    );
    Ok(())
}
