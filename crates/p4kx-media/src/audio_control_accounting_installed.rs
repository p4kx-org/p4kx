// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io::{self, Write as _};
use std::path::Path;
use std::time::Instant;

use p4kx_assembly::audio::AtlDefinitions;
use p4kx_extraction::{Archive, ArchiveWatch, Build};
use p4kx_testkit::{real_archive_b_path, real_archive_key, real_archive_path};

use super::{
    Census, StreamPhase, TestResult, privacy, require, source_counts, verify_production_stream,
};

#[derive(Debug)]
enum Phase {
    OpenArchive,
    ReadSources,
    ReconcileMedia,
    ComparePlaybackDefinitions,
    CheckArchive,
}

struct Verification {
    input: usize,
    phase: Phase,
    start: Instant,
}

impl Verification {
    fn enter(&mut self, phase: Phase) -> TestResult {
        self.phase = phase;
        let mut out = io::stdout().lock();
        writeln!(
            out,
            "Running ATL archive input {}: {:?} after {:.1}s",
            self.input,
            self.phase,
            self.start.elapsed().as_secs_f64()
        )?;
        out.flush()?;
        Ok(())
    }

    fn stream_phase(&self, phase: StreamPhase) -> TestResult {
        let mut out = io::stdout().lock();
        writeln!(
            out,
            "Running ATL archive input {}: {phase:?} after {:.1}s",
            self.input,
            self.start.elapsed().as_secs_f64()
        )?;
        out.flush()?;
        Ok(())
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn original_atl_census_reconciles_media_and_assembly_without_exports() -> Result<(), String> {
    let first = real_archive_path().map_err(|_| "missing configured ATL archive".to_owned())?;
    verify_installed(&first, 1)?;
    if let Some(second) = real_archive_b_path() {
        verify_installed(&second, 2)?;
    }
    Ok(())
}

fn verify_installed(path: &Path, input: usize) -> Result<(), String> {
    let mut verification = Verification {
        input,
        phase: Phase::OpenArchive,
        start: Instant::now(),
    };
    let result = (|| {
        verification.enter(Phase::OpenArchive)?;
        let archive = Archive::open_with_key(path, Some(real_archive_key()?))?;
        let build = archive.build().ok_or("authoritative installed Build")?;
        report_build(build)?;
        verify(&archive, build, &mut verification)
    })();
    privacy::redact(result).map_err(|_| {
        format!(
            "ATL archive input {input} failed during {:?}",
            verification.phase
        )
    })
}

fn verify(archive: &Archive, build: &Build, verification: &mut Verification) -> TestResult {
    let watch = ArchiveWatch::new(archive);
    verification.enter(Phase::ReadSources)?;
    let original = source_counts(archive, &watch)?;
    require(!original.rows.is_empty(), "root ATL documents present")?;

    verification.enter(Phase::ReconcileMedia)?;
    let _streamed =
        verify_production_stream(archive, &original, |phase| verification.stream_phase(phase))?;
    report_census(&original.rows)?;

    verification.enter(Phase::ComparePlaybackDefinitions)?;
    original
        .triggers
        .verify(archive, &AtlDefinitions::build(archive))?;
    verification.enter(Phase::CheckArchive)?;
    watch.check()?;
    let directory = archive
        .path()
        .parent()
        .ok_or("installed channel directory")?;
    require(
        Build::read_from(directory)?.as_ref() == Some(build),
        "Build unchanged throughout the census",
    )
}

fn report_build(build: &Build) -> TestResult {
    let mut out = io::stdout().lock();
    writeln!(
        out,
        "ATL census: Build {} {} {} {}",
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    )?;
    out.flush()?;
    Ok(())
}

fn report_census(original: &BTreeMap<usize, Census>) -> TestResult {
    let mut sum = Census::default();
    for source in original.values() {
        sum.documents += source.documents;
        sum.elements += source.elements;
        sum.declarations += source.declarations;
        sum.definitions += source.definitions;
        sum.edges += source.edges;
    }
    let mut out = io::stdout().lock();
    writeln!(
        out,
        "Original root ATL and serialised Media census: {sum:?}"
    )?;
    out.flush()?;
    Ok(())
}
