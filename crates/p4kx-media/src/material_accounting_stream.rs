// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::path::Path;
use std::time::Instant;

use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection, Filter};

use crate::checkpoint::Checkpoint;
use crate::input_budget::InputBudget;
use crate::media_writer::Finished;
use crate::origin_test_support::real_archive_privacy as privacy;
use crate::shader_lookup::ShaderLookup;
use crate::source_selection::SourceSelection;
use crate::traversal::Traversal;
use crate::{
    Algorithm, Artifact, ByteScope, ExportOptions, Float32, KindSummary, Limits, MaterialProvider,
    MediaKind, MediaRecord, MediaWriter, Provider, Source, Status, Tally, hashing, source,
};

use super::TestResult;

#[path = "material_accounting_census.rs"]
mod census;
#[path = "../tests/real_archive/material_accounting_fields.rs"]
mod fields;
#[path = "material_accounting_input.rs"]
mod input;
#[path = "material_accounting_parity.rs"]
mod parity;
#[path = "material_accounting_sink.rs"]
mod sink;

use census::CensusCounts;
use fields::Counts;
use input::{ParserOracle, Timings, Visit};
use sink::Sink;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn all_selected_material_libraries_and_raw_artifacts_reconcile()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify_configured())
}

fn verify_configured() -> TestResult {
    let primary = p4kx_testkit::real_archive_path()?;
    let secondary = p4kx_testkit::real_archive_b_path();
    let key = p4kx_testkit::real_archive_key()?;
    for (ordinal, path) in std::iter::once(primary).chain(secondary).enumerate() {
        let archive_number = ordinal + 1;
        let started = Instant::now();
        report_phase(archive_number, Phase::Open, started);
        let archive = Archive::open_with_key(path, Some(key.clone()))?;
        let result = verify(
            &archive,
            Limits::default(),
            ParserOracle::Observed,
            |phase| {
                report_phase(archive_number, phase, started);
            },
        )?;
        report(archive_number, &result.counts);
        report_timings(archive_number, &result.timings);
        require(
            result.counts.selected > 0,
            "selected material inventory is empty",
        )?;
    }
    Ok(())
}

struct Observed {
    finished: Finished,
    counts: Counts,
    entries: Tally,
    timings: Timings,
}

fn options(limits: Limits) -> TestResult<ExportOptions> {
    Ok(ExportOptions {
        kinds: vec![MediaKind::Material],
        include: Some(Filter::parse("*.mtl")?),
        diagnostic: true,
        limits,
        ..ExportOptions::default()
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Open,
    HashRoot,
    ReconcileStream,
    FinalizeStream,
    VerifyTotals,
}

fn verify(
    archive: &Archive,
    limits: Limits,
    oracle: ParserOracle,
    mut enter: impl FnMut(Phase),
) -> TestResult<Observed> {
    let watch = ArchiveWatch::new(archive);
    let options = options(limits)?;
    options.limits.validate()?;
    let protection = ArchiveWriteProtection::new(archive.path(), options.cancel.flag())?;
    let checkpoint = Checkpoint {
        cancel: &options.cancel,
        watch: &watch,
        protection: &protection,
    };
    enter(Phase::HashRoot);
    let (digest, size) = hashing::file(
        archive.path(),
        ByteScope::RootArchiveFile,
        limits.root_bytes,
        &checkpoint,
    )?;
    let mut root = source::root(archive, digest, size);
    if !root.enumeration_complete {
        return Err("material root inventory incomplete".into());
    }
    enter(Phase::ReconcileStream);
    let sink = Sink::new()?;
    let provider = MaterialProvider::default();
    let providers: [&dyn Provider; 1] = [&provider];
    let mut writer = MediaWriter::observed(
        Path::new("material-observation"),
        vec![provider.capability()],
        limits,
        checkpoint,
        options.diagnostic,
        &sink,
    )?;
    writer.origins.observe_root(archive, size)?;
    writer.bind_build(archive, &mut root)?;
    let mut traversed = collect(
        archive,
        &options,
        &root.digest,
        &providers,
        &sink,
        oracle,
        &mut writer,
    )?;
    enter(Phase::FinalizeStream);
    writer.index_build(&root)?;
    let finished = writer.finish()?;
    enter(Phase::VerifyTotals);
    traversed.timings.schema_decode = sink.schema_decode();
    let counts = sink.finish(&traversed.census)?;
    let [kind] = finished.kinds.as_slice() else {
        return Err("one material provider summary required".into());
    };
    counts.check_totals(&traversed.entries, kind)?;
    watch.check()?;
    Ok(Observed {
        finished,
        counts,
        entries: traversed.entries,
        timings: traversed.timings,
    })
}

struct Traversed {
    entries: Tally,
    census: CensusCounts,
    timings: Timings,
}

fn collect(
    archive: &Archive,
    options: &ExportOptions,
    root: &crate::Digest,
    providers: &[&dyn Provider],
    sink: &Sink,
    oracle: ParserOracle,
    writer: &mut MediaWriter<'_>,
) -> TestResult<Traversed> {
    let include = options.include.as_ref().ok_or("material include filter")?;
    let exclude = &options.exclude;
    let budget = InputBudget::default();
    let shaders = ShaderLookup::new(archive, root, SourceSelection { include, exclude }, &budget);
    let mut traversal = Traversal {
        root,
        providers,
        include,
        exclude,
        entries: Tally::default(),
        budget: &budget,
        index_keys: &Cell::new(0),
        shaders: Some(&shaders),
    };
    let mut visit = Visit::new(providers, &budget, sink, oracle);
    traversal.visit(archive, &mut Vec::new(), writer, &mut visit)?;
    let input::Survey {
        census,
        mut timings,
    } = visit.complete()?;
    timings.shader_index = shaders.index_time();
    Ok(Traversed {
        entries: traversal.entries,
        census,
        timings,
    })
}

fn require(condition: bool, message: &'static str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in test prints phase timing without source paths"
)]
fn report_phase(archive: usize, phase: Phase, started: Instant) {
    eprintln!(
        "material-source-export archive={archive} phase={phase:?} elapsed_ms={}",
        started.elapsed().as_millis()
    );
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in test prints anonymous coverage counts"
)]
fn report(archive: usize, counts: &Counts) {
    eprintln!(
        "material-source-export archive={archive} entries={} libraries={} nested={} complete={} partial={} failed={} materials={}",
        counts.enumerated,
        counts.selected,
        counts.nested,
        counts.complete,
        counts.partial,
        counts.failed,
        counts.materials,
    );
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in test prints aggregate timings without source paths"
)]
fn report_timings(archive: usize, timings: &Timings) {
    eprintln!(
        "material-source-export archive={archive} verified_payload_ms={} parse_ms={} projection_ms={} compare_ms={} shader_index_ms={} material_schema_decode_ms={}",
        timings.verified_payload.as_millis(),
        timings.parse.as_millis(),
        timings.projection.as_millis(),
        timings.compare.as_millis(),
        timings.shader_index.as_millis(),
        timings.schema_decode.as_millis(),
    );
}
