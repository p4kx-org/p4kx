// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Measures a consumer-owned mining cache against two installed Archive builds.

use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use p4kx_export::CatalogError;
use p4kx_export::mining_locations::{
    MiningLocationOptions, MiningLocationSnapshot, read_mining_locations,
};
use p4kx_extraction::{Archive, ArchiveKey, ArchiveWatch, Build, CancelToken, Language};
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const USAGE: &str = "usage: mining_location_consumer <old Data.p4k> <new Data.p4k> [language]";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refresh {
    Read,
    Reused,
}

struct CachedMining {
    path: PathBuf,
    watch: ArchiveWatch,
    snapshot: MiningLocationSnapshot,
}

struct ConsumerCache {
    key: ArchiveKey,
    current: Option<CachedMining>,
}

impl ConsumerCache {
    fn refresh(&mut self, path: &Path, options: MiningLocationOptions<'_>) -> Result<Refresh> {
        if options.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled.into());
        }
        let path = fs::canonicalize(path)?;
        if self.current.as_ref().is_some_and(|cached| {
            cached.path == path
                && cached.snapshot.language == options.language
                && cached.watch.check().is_ok()
        }) {
            return Ok(Refresh::Reused);
        }
        let archive = Archive::open_with_key(&path, Some(self.key.clone()))?;
        let watch = ArchiveWatch::new(&archive);
        watch.check()?;
        let build = archive.build().ok_or("a Build manifest is required")?;
        require_build(build)?;
        let snapshot = read_mining_locations(&archive, options)?;
        if snapshot.build.as_ref() != Some(build) || snapshot.language != options.language {
            return Err("the snapshot must identify the requested Build and locale".into());
        }
        watch.check()?;
        drop(archive);
        self.current = Some(CachedMining {
            path,
            watch,
            snapshot,
        });
        Ok(Refresh::Read)
    }

    fn snapshot(&self) -> Result<&MiningLocationSnapshot> {
        Ok(&self
            .current
            .as_ref()
            .ok_or("missing cached snapshot")?
            .snapshot)
    }
}

fn main() -> Result<()> {
    let mut args = env::args_os().skip(1);
    let old = PathBuf::from(args.next().ok_or(USAGE)?);
    let new = PathBuf::from(args.next().ok_or(USAGE)?);
    let language = args
        .next()
        .map(|value| value.into_string().map_err(|_| USAGE))
        .transpose()?
        .map_or(Ok(Language::English), |value| value.parse())?;
    if args.next().is_some() {
        return Err(USAGE.into());
    }
    let key = p4kx_testkit::real_archive_key()?;
    exercise(&old, &new, key, language)
}

fn exercise(old: &Path, new: &Path, key: ArchiveKey, language: Language) -> Result<()> {
    let mut cache = ConsumerCache { key, current: None };
    let options = MiningLocationOptions::new(language, None);
    let cold = measure(&mut cache, old, options, Refresh::Read, "cold")?;
    let changed = measure(&mut cache, new, options, Refresh::Read, "changedBuild")?;
    if cold == changed {
        return Err("the inputs must identify different Builds".into());
    }
    verify_cancelled_refresh(&mut cache, old, language, &changed)?;
    verify_failed_refresh(&mut cache, old, options, &changed)?;
    measure(&mut cache, new, options, Refresh::Reused, "warm")?;
    Ok(())
}

fn require_build(build: &Build) -> Result<()> {
    if [
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    ]
    .iter()
    .any(|field| field.trim().is_empty())
    {
        return Err("a complete Build identity is required".into());
    }
    Ok(())
}

fn measure(
    cache: &mut ConsumerCache,
    path: &Path,
    options: MiningLocationOptions<'_>,
    expected: Refresh,
    phase: &str,
) -> Result<Build> {
    let started = Instant::now();
    let observed = cache.refresh(path, options)?;
    let elapsed = started.elapsed();
    if observed != expected {
        return Err(format!("{phase}: expected {expected:?}, obtained {observed:?}").into());
    }
    let snapshot = cache.snapshot()?;
    let build = snapshot.build.as_ref().ok_or("missing cached Build")?;
    emit(&phase_report(phase, elapsed, expected, build, snapshot))?;
    Ok(build.clone())
}

fn verify_cancelled_refresh(
    cache: &mut ConsumerCache,
    old: &Path,
    language: Language,
    expected: &Build,
) -> Result<()> {
    let cancel = CancelToken::default();
    cancel.cancel();
    let started = Instant::now();
    let options = MiningLocationOptions::new(language, Some(&cancel));
    let result = cache.refresh(old, options);
    let elapsed = started.elapsed();
    if !matches!(result, Err(error) if matches!(error.downcast_ref::<CatalogError>(), Some(CatalogError::Cancelled)))
        || cache.snapshot()?.build.as_ref() != Some(expected)
    {
        return Err("cancellation must keep the last successful cache".into());
    }
    emit(&json!({
        "phase": "cancelled",
        "elapsedMillis": elapsed.as_millis(),
        "retainedBuildId": expected.build_id(),
    }))
}

fn verify_failed_refresh(
    cache: &mut ConsumerCache,
    old: &Path,
    options: MiningLocationOptions<'_>,
    expected: &Build,
) -> Result<()> {
    let invalid = old.join("Data.p4k");
    let started = Instant::now();
    let result = cache.refresh(&invalid, options);
    let elapsed = started.elapsed();
    if !matches!(result, Err(error) if error.downcast_ref::<io::Error>().is_some())
        || cache.snapshot()?.build.as_ref() != Some(expected)
    {
        return Err("an input failure must keep the last successful cache".into());
    }
    emit(&json!({
        "phase": "inputFailure",
        "elapsedMillis": elapsed.as_millis(),
        "retainedBuildId": expected.build_id(),
    }))
}

fn phase_report(
    phase: &str,
    elapsed: Duration,
    refresh: Refresh,
    build: &Build,
    snapshot: &MiningLocationSnapshot,
) -> Value {
    let coverage = &snapshot.coverage;
    json!({
        "phase": phase,
        "elapsedMillis": elapsed.as_millis(),
        "reused": refresh == Refresh::Reused,
        "build": {
            "branch": build.branch(),
            "version": build.version(),
            "changeNumber": build.change_number(),
            "id": build.build_id(),
        },
        "language": snapshot.language.code(),
        "rows": {
            "elements": snapshot.mineable_elements.len(),
            "compositions": snapshot.mineable_compositions.len(),
            "providers": snapshot.harvestable_provider_presets.len(),
            "locations": snapshot.harvestable_locations.len(),
            "occurrences": snapshot.harvestable_occurrences.len(),
            "starmapObjects": snapshot.starmap_objects.len(),
            "resources": snapshot.resource_types.len(),
            "publicNames": snapshot.public_names.len(),
            "gameTexts": snapshot.game_texts.len(),
        },
        "sources": snapshot.source_inventory.source_count(),
        "uses": snapshot.source_inventory.use_count(),
        "coverage": {
            "languageAvailable": coverage.language_available,
            "archiveInventoryIncomplete": coverage.archive_inventory_incomplete,
            "unreadableRecordHeaders": coverage.unreadable_record_headers,
            "unreadableRecordBodies": coverage.unreadable_record_bodies,
            "unreadableTextFields": coverage.unreadable_text_fields,
            "unreadableStarmapRelationships": coverage.unreadable_starmap_relationships,
            "unreadableMiningFields": coverage.unreadable_mining_fields,
            "unresolvedResourceLinks": coverage.unresolved_resource_links,
            "unreadableResourceTypes": coverage.unreadable_resource_types,
            "unreadableProviderLinks": coverage.unreadable_provider_links,
            "placementInventoryIncomplete": coverage.placement_inventory_incomplete,
            "unavailablePlacementOccurrences": coverage.unavailable_placement_occurrences,
            "unreadableHarvestContainers": coverage.unreadable_harvest_containers,
            "locationsWithUnresolvedParent": coverage.locations_with_unresolved_parent,
        },
    })
}

fn emit(report: &Value) -> Result<()> {
    use std::io::Write as _;

    let mut out = io::stdout().lock();
    serde_json::to_writer(&mut out, report)?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}
