// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::{Duration, Instant};

use super::*;
use crate::texture_budget::{Tally, Verdict};

fn neighbouring_jobs(count: usize) -> Vec<Job> {
    (0..count)
        .map(|number| {
            let path = format!("objects/s{}_ddna.dds", number.div_ceil(2));
            Job {
                key: Key::Normal {
                    normal: format!("{path}#{number}"),
                },
                diffuse: None,
                normal: Some(source(path, true)),
                specular: None,
                explicit_gloss: Vec::new(),
                tints: TINTS,
                layered: None,
            }
        })
        .collect()
}

fn job_of(path: &str) -> Job {
    Job {
        key: Key::Normal {
            normal: path.to_owned(),
        },
        diffuse: None,
        normal: Some(source(path.to_owned(), true)),
        specular: None,
        explicit_gloss: Vec::new(),
        tints: TINTS,
        layered: None,
    }
}

fn run_holding(jobs: &[Job], loader: &Squares, held: u64) -> Vec<Outcome> {
    run_with(jobs, SourceStore::new(loader, 0, held), Budget::DEFAULT)
}

#[test]
fn sources_shared_by_neighbouring_jobs_are_decoded_once_though_the_decode_is_parallel() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .expect("a pool");
    let worker = std::thread::spawn(move || {
        let loader = Squares::new(4).sized().parallel();
        let outcomes = pool.install(|| run(&neighbouring_jobs(64), 0, &loader, Budget::DEFAULT));
        (made(&outcomes), loader.loads())
    });
    let deadline = Instant::now() + Duration::from_secs(60);
    while !worker.is_finished() {
        assert!(
            Instant::now() < deadline,
            "the build waits on a shared source instead of finishing"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    let (images, loads) = worker
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));

    assert_eq!(images, 64, "every image is made");
    assert_eq!(loads, 33, "one decode per distinct source");
}

#[test]
fn packed_gloss_reserves_a_fitting_specular_size_and_keeps_fallback_possible() {
    let job = Job {
        key: Key::MetallicRoughness {
            gloss: None,
            explicit_gloss: vec!["coat_gloss.dds".to_owned()],
            specular: Some(("coat_spec.dds".to_owned(), [u8::MAX; 3])),
        },
        diffuse: None,
        normal: None,
        specular: Some(source("coat_spec.dds".to_owned(), false)),
        explicit_gloss: vec![source("coat_gloss.dds".to_owned(), false)],
        tints: TINTS,
        layered: None,
    };
    let sized = Squares::new(4).sized();
    let mut store = SourceStore::new(&sized, 0, 0);
    let mut ample = Tally::new(Budget {
        images: 1,
        pixels: 16,
    });
    assert!(matches!(
        ample.plan(&job, &mut store),
        Verdict::Reserved(16)
    ));

    let mut smaller = Tally::new(Budget {
        images: 1,
        pixels: 15,
    });
    assert!(matches!(smaller.plan(&job, &mut store), Verdict::Unknown));

    let unreadable = Squares::new(4).unsizable();
    let mut store = SourceStore::new(&unreadable, 0, 0);
    assert!(matches!(smaller.plan(&job, &mut store), Verdict::Unknown));
}

#[test]
fn a_group_of_jobs_decodes_no_more_pixels_than_the_limit() {
    let loader = Squares::new(4).sized();
    let jobs = normal_jobs(8);
    let mut store = SourceStore::new(&loader, 0, 0).decoding(40);

    assert_eq!(
        store.group_len(&jobs, 4),
        2,
        "two sources of 16 pixels fit 40"
    );
    assert_eq!(store.group_len(&[], 4), 0);
}

#[test]
fn a_source_shared_inside_a_group_counts_once() {
    let loader = Squares::new(4).sized();
    let jobs = neighbouring_jobs(8);
    let mut store = SourceStore::new(&loader, 0, 0).decoding(32);

    assert_eq!(
        store.group_len(&jobs, 4),
        3,
        "s0, s1 and s1 again: 32 pixels"
    );
}

#[test]
fn a_job_larger_than_the_limit_or_of_unknown_size_runs_alone() {
    let sized = Squares::new(4).sized();
    let unsized_loader = Squares::new(4);
    let unreadable = Squares::new(4).unsizable();
    let mut small = SourceStore::new(&sized, 0, 0).decoding(8);
    let mut unknown = SourceStore::new(&unsized_loader, 0, 0);
    let mut failing = SourceStore::new(&unreadable, 0, 0);

    assert_eq!(small.group_len(&normal_jobs(4), 4), 1);
    assert_eq!(unknown.group_len(&normal_jobs(4), 4), 1);
    assert_eq!(failing.group_len(&normal_jobs(4), 4), 1);
}

#[test]
fn a_build_under_a_small_limit_makes_every_image() {
    let loader = Squares::new(4).sized();
    let store = SourceStore::new(&loader, 0, 0).decoding(20);

    let outcomes = run_with(&neighbouring_jobs(9), store, Budget::DEFAULT);

    assert_eq!(made(&outcomes), 9, "every image is made");
    assert_eq!(loader.loads(), 5, "one decode per distinct source");
}

#[test]
fn a_source_the_next_group_needs_is_held_only_within_the_cap() {
    let kept = Squares::new(4).sized();
    let dropped = Squares::new(4).sized();

    run_holding(&neighbouring_jobs(8), &kept, 16);
    run_holding(&neighbouring_jobs(8), &dropped, 15);

    assert_eq!(
        kept.loads(),
        5,
        "the source across the group boundary is held"
    );
    assert_eq!(dropped.loads(), 6, "over the cap it is decoded again");
}

#[test]
fn a_held_source_survives_a_group_that_does_not_use_it() {
    let loader = Squares::new(4).sized();
    let jobs = [job_of("a.dds"), job_of("b.dds"), job_of("a.dds")];
    let store = SourceStore::new(&loader, 0, 16).decoding(16);

    let outcomes = run_with(&jobs, store, Budget::DEFAULT);

    assert_eq!(made(&outcomes), 3, "every image is made");
    assert_eq!(loader.loads(), 2, "a.dds is held across the group of b.dds");
}

#[test]
fn the_held_sources_never_exceed_the_cap() {
    let loader = Squares::new(4);
    let jobs = neighbouring_jobs(8);
    let mut store = SourceStore::new(&loader, 0, 16);

    let decoded = store.prepare(&jobs[..4]);
    store.keep(decoded, &jobs[4..]);

    assert!(store.held_pixels() <= 16, "{}", store.held_pixels());
    assert!(store.held_pixels() > 0, "the shared source is held");
}
