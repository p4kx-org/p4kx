// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Byte comparison of the manifest-selected Entries unp4k extracted.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::time::Instant;

use p4kx_extraction::{Archive, Compression, EffectiveEncoding, Entry};
use p4kx_testkit::{real_archive_key, real_archive_path, reference_unp4k_path};
use rayon::prelude::*;

use super::TestResult;
use super::bytes::{Verdict, compare_streams};
use super::reference_artifacts;
use super::reference_error::Result;
use super::reference_manifest::{Expected, Location, PatchScope, Tool};
use super::reference_paths;
use super::reference_reader::Fingerprinted;
use super::reference_run::{self, Dataset};
use super::tally::Tally;
use super::unp4k_allow::{self, Context, SourceIdentity, TRAILING_ZEROS};

type Kind = (Compression, bool);

#[derive(Default)]
struct Outcome {
    tally: Tally,
    kinds: BTreeMap<String, (u64, u64)>,
}

impl Outcome {
    fn merge(mut self, other: Self) -> Self {
        self.tally = self.tally.merge(other.tally);
        for (kind, (compared, equal)) in other.kinds {
            let slot = self.kinds.entry(kind).or_default();
            slot.0 += compared;
            slot.1 += equal;
        }
        self
    }
}

fn kind_name(kind: Kind) -> String {
    format!(
        "{:?} {}",
        kind.0,
        if kind.1 { "encrypted" } else { "plain" }
    )
}

/// Records an exact comparison or an evidenced producer-specific difference.
pub(crate) fn judge(
    tally: &mut Tally,
    source: SourceIdentity<'_>,
    encoding: EffectiveEncoding,
    context: Option<&Context<'_>>,
    verdict: Verdict,
) -> bool {
    match verdict {
        Verdict::Equal => {
            tally.equal();
            true
        }
        allowed if unp4k_allow::allows(context, source, encoding, allowed) => {
            tally.allow(TRAILING_ZEROS);
            false
        }
        other => {
            tally.different(format!("{}: {other:?}", source.path));
            false
        }
    }
}

fn compare_file(
    archive: &Archive,
    entry: &Entry,
    dataset: &Dataset,
    target: &Location,
) -> std::result::Result<(Verdict, EffectiveEncoding), String> {
    let artifact = dataset
        .artifact(&target.path)
        .map_err(|error| error.to_string())?;
    let expected = reference_artifacts::fingerprint(artifact).map_err(|error| error.to_string())?;
    let path = reference_paths::resolve(&dataset.root, &artifact.path)
        .map_err(|error| error.to_string())?;
    let reference = File::open(path).map_err(|error| error.to_string())?;
    let mut reference = Fingerprinted::new(reference, expected.bytes);
    let mut ours = archive
        .open_entry(entry)
        .map_err(|error| error.to_string())?;
    let verdict = compare_streams(&mut ours, &mut reference).map_err(|error| error.to_string())?;
    if !matches!(verdict, Verdict::Differs { .. }) {
        reference
            .finish(expected)
            .map_err(|error| error.to_string())?;
    }
    Ok((verdict, ours.effective_encoding()))
}

fn one(archive: &Archive, dataset: &Dataset, expected: &Expected) -> Outcome {
    let mut outcome = Outcome::default();
    let source = dataset
        .source
        .entry(archive, &expected.identity)
        .and_then(|entry| {
            expected
                .targets
                .first()
                .map(|target| (entry, target))
                .ok_or_else(|| {
                    super::reference_error::ReferenceError::new(
                        super::reference_error::Code::Coverage,
                        "selected source has no target",
                    )
                })
        });
    let (entry, target) = match source {
        Ok(source) => source,
        Err(error) => {
            outcome.tally.different(error.to_string());
            return outcome;
        }
    };
    let producer = &dataset.manifest.producer;
    let patch_scope = match &producer.patch_scope {
        PatchScope::Unmodified => None,
        PatchScope::Local { description, .. } => Some(description.as_str()),
    };
    let context = Context {
        revision: &producer.revision,
        patch_scope,
        source: SourceIdentity::of(entry),
    };
    let equal = match compare_file(archive, entry, dataset, target) {
        Ok((verdict, encoding)) => judge(
            &mut outcome.tally,
            SourceIdentity::of(entry),
            encoding,
            Some(&context),
            verdict,
        ),
        Err(error) => {
            outcome.tally.different(error);
            false
        }
    };
    outcome.kinds.insert(
        kind_name((entry.compression(), entry.is_encrypted())),
        (1, u64::from(equal)),
    );
    outcome
}

pub(crate) fn verify(archive: &Archive, root: &Path) -> Result<Tally> {
    let dataset = Dataset::load(root, archive, Tool::Unp4k)?;
    let outcome = dataset
        .manifest
        .expected
        .par_iter()
        .map(|expected| one(archive, &dataset, expected))
        .reduce(Outcome::default, Outcome::merge);
    for (kind, (compared, equal)) in &outcome.kinds {
        super::report(&format!("  {kind}: {compared} compared, {equal} equal"));
    }
    dataset.finish(archive, &outcome.tally, "unp4k byte comparison")?;
    Ok(outcome.tally)
}

#[test]
#[ignore = "real Archive and reference output: set P4KX_DATA_P4K and P4KX_REF_UNP4K and pass --ignored"]
fn every_entry_unp4k_extracted_is_byte_identical() -> TestResult {
    reference_run::optional(reference_unp4k_path(), "P4KX_REF_UNP4K", |root| {
        let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
        let started = Instant::now();
        verify(&archive, root)?;
        super::report(&format!("unp4k run took {:?}", started.elapsed()));
        Ok(())
    })
}
