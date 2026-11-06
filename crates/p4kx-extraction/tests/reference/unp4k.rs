// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Byte comparison of every Entry unp4k extracted with what P4KX decodes.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::time::Instant;

use p4kx_extraction::{Archive, Compression, EffectiveEncoding, Entry};
use p4kx_testkit::{real_archive_key, real_archive_path, reference_unp4k_path};
use rayon::prelude::*;

use super::TestResult;
use super::bytes::{Verdict, compare_streams};
use super::tally::Tally;
use super::unp4k_allow::{self, Context, SourceIdentity, TRAILING_ZEROS};
use super::walk::{files_under, index_entries, relative_key};

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
    file: &Path,
) -> Result<(Verdict, EffectiveEncoding), String> {
    let reference = File::open(file).map_err(|error| format!("{}: {error}", file.display()))?;
    let mut ours = archive
        .open_entry(entry)
        .map_err(|error| format!("{}: {error}", entry.path()))?;
    let verdict = compare_streams(&mut ours, reference)
        .map_err(|error| format!("{}: {error}", entry.path()))?;
    Ok((verdict, ours.effective_encoding()))
}

fn run(archive: &Archive, root: &Path) -> Result<Outcome, std::io::Error> {
    let files = files_under(root)?;
    let index = index_entries(archive.entries());
    let outcome = files
        .par_iter()
        .map(|file| {
            let mut outcome = Outcome::default();
            let Some(key) = relative_key(root, file) else {
                return outcome;
            };
            let Some(entry) = index.get(&key) else {
                outcome
                    .tally
                    .missing_in_archive(format!("{key}: no such Entry"));
                return outcome;
            };
            let kind = kind_name((entry.compression(), entry.is_encrypted()));
            let equal = match compare_file(archive, entry, file) {
                Ok((verdict, encoding)) => judge(
                    &mut outcome.tally,
                    SourceIdentity::of(entry),
                    encoding,
                    None,
                    verdict,
                ),
                Err(message) => {
                    outcome.tally.different(message);
                    false
                }
            };
            let slot = outcome.kinds.entry(kind).or_default();
            slot.0 += 1;
            slot.1 += u64::from(equal);
            outcome
        })
        .reduce(Outcome::default, Outcome::merge);
    Ok(outcome)
}

#[test]
#[ignore = "real Archive and reference output: set P4KX_DATA_P4K and P4KX_REF_UNP4K and pass --ignored"]
fn every_entry_unp4k_extracted_is_byte_identical() -> TestResult {
    let Some(root) = reference_unp4k_path() else {
        super::report("skipped: P4KX_REF_UNP4K is not set");
        return Ok(());
    };
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let started = Instant::now();

    let outcome = run(&archive, &root)?;

    for (kind, (compared, equal)) in &outcome.kinds {
        super::report(&format!("  {kind}: {compared} compared, {equal} equal"));
    }
    super::report(&format!("unp4k run took {:?}", started.elapsed()));
    outcome.tally.finish("unp4k byte comparison")?;
    Ok(())
}
