// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The layer files read as libraries: one fetch per key, one library path, bounded by the
//! estimated size of what is held, and shared by threads.

use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_model::{CompanionError, CompanionPath, LayerCache, LayerLimits, MaterialStatus};

use super::layer_cache::{FIRST, SET, TestResult, hull, load, set};

#[test]
fn a_layer_file_read_as_a_library_is_fetched_once_with_the_surface_loads() -> TestResult {
    let fetched = Cell::new(0);
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Material(FIRST) => Some(hull("first")),
            CompanionPath::Material(SET) => {
                fetched.set(fetched.get() + 1);
                Some(set())
            }
            _ => None,
        })
    };
    let cache = LayerCache::new();
    let mut budget = cache.budget();

    load(FIRST, &mut companions, &cache);
    let status = cache.library(
        r"Data\materials\layers\SET.mtl",
        &mut companions,
        &mut budget,
    );
    let again = cache.library(SET, &mut companions, &mut budget);

    let library = status.library().ok_or("loaded")?;
    assert_eq!(library.materials.len(), 2, "every sub-material is there");
    assert!(Arc::ptr_eq(&status, &again), "one parsed file");
    assert_eq!(fetched.get(), 1, "one fetch for the load and both reads");
    Ok(())
}

#[test]
fn a_layer_library_that_cannot_be_read_is_a_status_with_its_reason() {
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Material("materials/layers/broken") => Some(b"not xml".to_vec()),
            _ => None,
        })
    };
    let cache = LayerCache::with_limits(LayerLimits {
        files_per_load: 2,
        ..LayerLimits::default()
    });
    let mut budget = cache.budget();

    let missing = cache.library("materials/layers/gone.mtl", &mut companions, &mut budget);
    let broken = cache.library("materials/layers/broken.mtl", &mut companions, &mut budget);
    let spent = cache.library("materials/layers/third.mtl", &mut companions, &mut budget);
    let unsafe_path = cache.library("../outside.mtl", &mut companions, &mut budget);

    assert!(matches!(*missing, MaterialStatus::Missing { .. }));
    assert!(matches!(*broken, MaterialStatus::Unreadable { .. }));
    assert!(spent.problem().is_some_and(|text| text.contains("budget")));
    assert!(matches!(*unsafe_path, MaterialStatus::Unreadable { .. }));
    assert_eq!(
        cache.len(),
        2,
        "the unadmitted and unsafe files are not cached"
    );
}

#[test]
fn two_spellings_of_a_layer_path_are_one_file_with_one_library_path() -> TestResult {
    let fetched = Cell::new(0);
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Material(path) if path.eq_ignore_ascii_case(SET) => {
                fetched.set(fetched.get() + 1);
                Some(set())
            }
            _ => None,
        })
    };
    let cache = LayerCache::new();
    let mut budget = cache.budget();

    let first = cache.library(
        r"Data\Materials\Layers\SET.mtl",
        &mut companions,
        &mut budget,
    );
    let second = cache.library("materials/layers/set", &mut companions, &mut budget);

    assert_eq!(fetched.get(), 1, "one fetch");
    assert!(Arc::ptr_eq(&first, &second), "one library");
    assert_eq!(first.library().ok_or("loaded")?.path, SET, "a fixed path");
    Ok(())
}

#[test]
fn a_cache_starts_over_when_the_parsed_libraries_it_holds_reach_the_byte_limit() {
    let mut companions =
        |_: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> { Ok(Some(set())) };
    let cache = LayerCache::with_limits(LayerLimits {
        cached_bytes: 1,
        ..LayerLimits::default()
    });
    let mut budget = cache.budget();

    drop(cache.library("materials/layers/a", &mut companions, &mut budget));
    drop(cache.library("materials/layers/b", &mut companions, &mut budget));

    assert_eq!(cache.len(), 1, "a made room for b");
}

#[test]
fn threads_asking_for_one_layer_file_fetch_it_once_and_share_one_library() {
    const THREADS: usize = 8;
    let fetched = AtomicUsize::new(0);
    let cache = LayerCache::new();

    let found: Vec<Arc<MaterialStatus>> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..THREADS)
            .map(|_| {
                scope.spawn(|| {
                    let mut companions =
                        |_: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
                            fetched.fetch_add(1, Ordering::SeqCst);
                            Ok(Some(set()))
                        };
                    cache.library(SET, &mut companions, &mut cache.budget())
                })
            })
            .collect();
        workers
            .into_iter()
            .filter_map(|worker| worker.join().ok())
            .collect()
    });

    assert_eq!(found.len(), THREADS);
    assert_eq!(fetched.load(Ordering::SeqCst), 1, "one fetch for all");
    assert!(found.windows(2).all(|pair| Arc::ptr_eq(&pair[0], &pair[1])));
}
