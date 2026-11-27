// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Value comparison of the manifest-selected DataForge Records `StarBreaker` wrote.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use p4kx_extraction::{Archive, DataForge, DataForgeEntry, TreeBuilder};
use p4kx_testkit::{real_archive_key, real_archive_path, reference_starbreaker_path};
use rayon::prelude::*;
use serde_json::Value;

use super::TestResult;
use super::reference_error::{Code, ReferenceError, Result};
use super::reference_manifest::{Identity, Location, Tool};
use super::reference_run::{self, Dataset};
use super::starbreaker_location;
use super::tally::{Compared, Tally};

type Group<'a> = Vec<(usize, &'a Location)>;
type Comparison = (usize, Compared, u64);

fn groups(dataset: &Dataset) -> Result<BTreeMap<String, Group<'_>>> {
    let mut groups = BTreeMap::<String, Group<'_>>::new();
    for (index, expected) in dataset.manifest.expected.iter().enumerate() {
        for target in &expected.targets {
            let key = super::reference_paths::key(&target.path)?;
            groups.entry(key).or_default().push((index, target));
        }
    }
    Ok(groups)
}

fn document(dataset: &Dataset, group: &Group<'_>) -> std::result::Result<Value, String> {
    let (_, location) = group.first().ok_or("empty output group")?;
    let bytes = dataset
        .read(&location.path)
        .map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

fn compare_group<'f>(
    forge: &'f DataForge<'f>,
    builder: &mut TreeBuilder<'f>,
    dataset: &Dataset,
    group: &Group<'_>,
) -> Vec<Comparison> {
    let document = document(dataset, group);
    group
        .iter()
        .map(|(index, location)| {
            let result = dataset.manifest.expected.get(*index).and_then(|expected| {
                if let Identity::Record { guid, .. } = &expected.identity {
                    Some(guid)
                } else {
                    None
                }
            });
            let (compared, embedded) = match (&document, result) {
                (Ok(document), Some(guid)) => {
                    starbreaker_location::compare(forge, builder, guid, location, document)
                }
                (Err(error), _) => (Compared::Different(error.clone()), 0),
                _ => (
                    Compared::Different("mapping has no Record identity".to_owned()),
                    0,
                ),
            };
            (*index, compared, embedded)
        })
        .collect()
}

fn merge(left: Compared, right: Compared) -> Compared {
    match (left, right) {
        (different @ Compared::Different(_), _) | (_, different @ Compared::Different(_)) => {
            different
        }
        (Compared::Allowed(mut left), Compared::Allowed(right)) => {
            left.extend(right);
            Compared::Allowed(left)
        }
        (allowed @ Compared::Allowed(_), _) | (_, allowed @ Compared::Allowed(_)) => allowed,
        _ => Compared::Equal,
    }
}

fn collect(comparisons: Vec<Vec<Comparison>>) -> Tally {
    let mut sources = BTreeMap::<usize, (Compared, u64)>::new();
    for (index, compared, embedded) in comparisons.into_iter().flatten() {
        let (old, count) = sources.remove(&index).unwrap_or((Compared::Equal, 0));
        sources.insert(index, (merge(old, compared), count + embedded));
    }
    let mut tally = Tally::default();
    for (index, (compared, embedded)) in sources {
        tally.record(&format!("logical source {index}"), compared);
        tally.embedded_records += embedded;
    }
    tally
}

pub(crate) fn verify<'f>(
    archive: &Archive,
    forge: &'f DataForge<'f>,
    root: &Path,
) -> Result<Tally> {
    let dataset = Dataset::load(root, archive, Tool::Starbreaker)?;
    let groups = groups(&dataset)?;
    let comparisons = groups
        .par_iter()
        .map_init(TreeBuilder::new, |builder, (_, group)| {
            compare_group(forge, builder, &dataset, group)
        })
        .collect();
    let tally = collect(comparisons);
    dataset.finish(archive, &tally, "StarBreaker value comparison")?;
    Ok(tally)
}

fn export_all<'f>(forge: &'f DataForge<'f>) -> (u64, u64) {
    let built = AtomicU64::new(0);
    let failed = AtomicU64::new(0);
    (0..forge.record_count())
        .into_par_iter()
        .for_each_init(TreeBuilder::new, |builder, index| {
            let result = u32::try_from(index)
                .ok()
                .and_then(|index| forge.record(index))
                .map(|found| builder.build(&found).is_ok());
            match result {
                Some(true) => built.fetch_add(1, Ordering::Relaxed),
                _ => failed.fetch_add(1, Ordering::Relaxed),
            };
        });
    (built.into_inner(), failed.into_inner())
}

#[test]
#[ignore = "real Archive and reference output: set P4KX_DATA_P4K and P4KX_REF_STARBREAKER and pass --ignored"]
fn every_record_starbreaker_wrote_has_the_same_values() -> TestResult {
    reference_run::optional(
        reference_starbreaker_path(),
        "P4KX_REF_STARBREAKER",
        |root| {
            let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
            let started = Instant::now();
            let entry = DataForgeEntry::read(&archive)?;
            let forge = entry.open()?;
            verify(&archive, &forge, root)?;
            super::report(&format!(
                "StarBreaker comparison took {:?}",
                started.elapsed()
            ));
            let (built, failed) = export_all(&forge);
            super::report(&format!(
                "separate self-check: {built} Record trees built, {failed} failed"
            ));
            if failed > 0 {
                return Err(
                    ReferenceError::new(Code::Coverage, "source Record self-check failed").into(),
                );
            }
            Ok(())
        },
    )
}
