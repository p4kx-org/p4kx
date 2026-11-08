// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Opt-in nested shader source and export accounting, without private fixture names.

use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, CancelToken};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind};

use super::{TestResult, privacy, shader_artifacts, shader_inventory, shader_record_accounting};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn nested_shader_sources_properties_and_raw_artifacts_reconcile()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify_configured())
}

fn verify_configured() -> TestResult {
    verify_path(&p4kx_testkit::real_archive_path()?)?;
    if let Some(second) = p4kx_testkit::real_archive_b_path() {
        verify_path(&second)?;
    }
    Ok(())
}

fn verify_path(path: &Path) -> TestResult {
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let cancel = CancelToken::new();
    let selection = shader_inventory::discover(&archive, &watch, &cancel)?;
    let shader_inventory::Selection {
        include,
        exclude,
        sources,
        enumerated,
        compiled,
    } = selection;
    let mut options = ExportOptions {
        kinds: vec![MediaKind::Shader],
        include: Some(include),
        exclude,
        cancel,
        diagnostic: true,
        ..ExportOptions::default()
    };
    options.limits.entry_bytes = shader_inventory::CONTAINER_BYTES;
    options.limits.source_bytes = shader_inventory::EXPORT_BYTES;
    options.limits.sources = shader_inventory::ENTRY_COUNT;
    options.limits.output_bytes = shader_artifacts::OUTPUT_BYTES;
    options.limits.records = shader_record_accounting::MAX_RECORDS;
    let folder = tempfile::tempdir()?;
    let result = p4kx_media::export(&archive, folder.path(), &options)?;
    let (index, output) = match result {
        ExportOutcome::Published { index, .. } => (index, folder.path().to_path_buf()),
        ExportOutcome::Incomplete { index, diagnostic } => (
            index,
            diagnostic
                .parent()
                .ok_or("diagnostic directory")?
                .to_path_buf(),
        ),
        _ => return Err("shader verification did not execute an export".into()),
    };
    watch.check()?;
    let retained = shader_artifacts::verify(&output, &index, &sources, &watch)?;
    let counts =
        shader_record_accounting::verify(&output, &index, &sources, &retained, enumerated, &watch)?;
    report(
        counts.definitions,
        counts.properties,
        counts.failed,
        counts.partial,
        compiled,
    );
    if counts.failed != 0 || counts.partial != 0 {
        return Err("shader accounting is explicit but selected sources remain incomplete".into());
    }
    folder.close()?;
    watch.check()?;
    Ok(())
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in test reports only aggregate coverage without private source identities"
)]
fn report(definitions: u64, properties: u64, failed: u64, partial: u64, compiled: usize) {
    eprintln!(
        "nested .pak shader accounting; .socpak explicitly excluded: definitions={definitions}, properties={properties}, failed={failed}, partial={partial}, compiled_sources_unselected={compiled}"
    );
}

pub(super) fn require(condition: bool, message: &'static str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
