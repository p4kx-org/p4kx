// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::path::Path;

use p4kx_extraction::{
    Archive, ArchiveWatch, ArchiveWriteProtection, Build, CancelToken, Exclusion, Filter,
    SourceUses,
};

use crate::checkpoint::Checkpoint;
use crate::input_budget::InputBudget;
use crate::origin_test_support::real_archive_privacy as privacy;
use crate::traversal::Traversal;
use crate::{
    Artifact, ByteOrder, ByteScope, ChunkReference, DataType, Digest, Limits, MediaRecord,
    MediaWriter, OffsetBasis, Provider, Source, Status, Tally, hashing, source,
};

use super::AnimationProvider;

#[path = "installed_failure.rs"]
mod failure;
#[path = "../../tests/real_archive/animation_fixture.rs"]
mod fixture;
#[path = "installed_origin_batches.rs"]
mod origin_batches;
#[path = "installed_projection.rs"]
mod projection;
#[path = "../../tests/real_archive/animation_rows.rs"]
mod rows;
#[path = "installed_stream.rs"]
mod stream;

use projection::Visit;
use rows::{Family, Totals};
use stream::Observation;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn animation_observation_reconciles_complete_caf_dba_and_aim_sources() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let archive = fixture::complete(&temporary.path().join("Data.p4k"))?;
    let totals = verify(&archive, &CancelToken::new(), Limits::default())?;
    for (expected, actual) in fixture::EXPECTED.iter().zip(&totals) {
        expected
            .check(actual)
            .map_err(|cause| format!("{} fixture: {cause}", expected.family.suffix()))?;
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn all_selected_animation_entries_clips_and_keys_reconcile()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify_configured())
}

fn verify_configured() -> TestResult {
    let primary = p4kx_testkit::real_archive_path()?;
    let secondary = p4kx_testkit::real_archive_b_path();
    let key = p4kx_testkit::real_archive_key()?;
    for (position, path) in std::iter::once(primary).chain(secondary).enumerate() {
        let archive = Archive::open_with_key(path, Some(key.clone()))?;
        let expected = archive.build().ok_or("installed animation Build")?;
        report_build(position + 1, expected);
        let totals = verify(&archive, &CancelToken::new(), Limits::default())?;
        require(
            totals.iter().any(|total| total.clips > 0)
                && totals
                    .iter()
                    .all(|total| total.complete && total.partial == 0),
            "installed animation inventory is empty or incomplete",
        )?;
        let parent = archive
            .path()
            .parent()
            .ok_or("installed animation directory")?;
        require(
            Build::read_from(parent)?.as_ref() == Some(expected),
            "unchanged installed animation Build",
        )?;
    }
    Ok(())
}

fn verify(archive: &Archive, cancel: &CancelToken, limits: Limits) -> TestResult<[Totals; 3]> {
    limits.validate()?;
    let watch = ArchiveWatch::new(archive);
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let checkpoint = Checkpoint {
        cancel,
        watch: &watch,
        protection: &protection,
    };
    report_phase("hash-root");
    let (digest, size) = hashing::file(
        archive.path(),
        ByteScope::RootArchiveFile,
        limits.root_bytes,
        &checkpoint,
    )?;
    checkpoint.finish()?;
    let schema = jsonschema::validator_for(&crate::builtin_record_schema()?)?;
    let run = Census {
        archive,
        checkpoint,
        limits,
        digest,
        size,
        schema: &schema,
    };
    let totals = [
        run.family(Family::Caf)?,
        run.family(Family::Dba)?,
        run.family(Family::Aim)?,
    ];
    checkpoint.finish()?;
    Ok(totals)
}

struct Census<'a> {
    archive: &'a Archive,
    checkpoint: Checkpoint<'a>,
    limits: Limits,
    digest: Digest,
    size: u64,
    schema: &'a jsonschema::Validator,
}

impl Census<'_> {
    fn family(&self, family: Family) -> TestResult<Totals> {
        report_phase(family.suffix());
        let observation = Observation::default();
        let provider = AnimationProvider;
        let providers: [&dyn Provider; 1] = [&provider];
        let mut writer = MediaWriter::observed(
            Path::new("animation-observation"),
            vec![provider.capability()],
            self.limits,
            self.checkpoint,
            true,
            &observation,
        )?;
        writer.origins.owner = SourceUses::with_batched_use_refs();
        let mut root = source::root(self.archive, self.digest.clone(), self.size);
        require(
            root.enumeration_complete,
            "original animation root enumeration is incomplete",
        )?;
        writer.origins.observe_root(self.archive, self.size)?;
        writer.bind_build(self.archive, &mut root)?;
        let include = Filter::parse(&format!("*.{}", family.suffix()))?;
        let budget = InputBudget::default();
        let mut traversal = Traversal {
            root: &root.digest,
            providers: &providers,
            include: &include,
            exclude: &Exclusion::default(),
            entries: Tally::default(),
            budget: &budget,
            index_keys: &Cell::new(0),
            shaders: None,
        };
        let mut visit = Visit::new(family, &providers, &budget, &observation, self.schema);
        failure::checked(
            family,
            "traversal",
            traversal.visit(self.archive, &mut Vec::new(), &mut writer, &mut visit),
        )?;
        failure::checked(family, "index-build", writer.index_build(&root))?;
        failure::checked(
            family,
            "reconcile-build-origins",
            observation.origins.finish_build(&root, &writer),
        )?;
        let finished = failure::checked(family, "finish-writer", writer.finish())?;
        let [kind] = finished.kinds.as_slice() else {
            return Err("one original animation provider required".into());
        };
        let totals = failure::checked(
            family,
            "reconcile-census",
            visit.finish(&traversal.entries, kind),
        )?;
        failure::checked(
            family,
            "reconcile-artifacts",
            observation.finish_census(&kind.counts.converted_outputs),
        )?;
        report(family, &totals);
        Ok(totals)
    }
}

fn require(condition: bool, message: &'static str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(failure::VerificationCheck(message).into())
    }
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in test reports the Build to which aggregate counts belong"
)]
fn report_build(position: usize, build: &Build) {
    eprintln!(
        "animation archive={position} build={} {} {} {}",
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    );
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in census reports its active phase without source paths"
)]
fn report_phase(phase: &str) {
    eprintln!("animation phase={phase}");
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in census reports anonymous aggregate counts"
)]
fn report(family: Family, totals: &Totals) {
    eprintln!(
        "animation {} entries={} chunks={} clips={} metadata_only={} controllers={} declared_keys={} decoded_keys={} partial_rows={} header_budget_refusals={}",
        family.suffix(),
        totals.entries,
        totals.chunks,
        totals.clips,
        totals.metadata_only_clips,
        totals.controllers,
        totals.declared,
        totals.decoded,
        totals.partial,
        totals.header_budget_refusals,
    );
}
