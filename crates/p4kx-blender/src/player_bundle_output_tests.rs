// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_extraction::DecimalU64;

use super::Output;
use super::wire::{self, Manifest, Stream, Totals};
use crate::{PackageError, PackageWriteContext};

type Check = Result<(), Box<dyn std::error::Error>>;

#[test]
fn cancelled_precommit_discards_only_owned_files_and_never_publishes() -> Check {
    let workspace = tempfile::tempdir()?;
    let path = workspace.path().join("bundle");
    let cancelled = AtomicBool::new(false);
    let mut output = Output::create(
        &path,
        PackageWriteContext::standalone(&|| false, &cancelled),
    )?;
    let prepared = manifest(&mut output)?;
    assert!(
        !path.join(wire::MANIFEST).exists(),
        "rows alone are not a published bundle"
    );
    cancelled.store(true, Ordering::Relaxed);
    let publish = output.publish(&prepared);
    assert!(matches!(
        output.finish(publish),
        Err(PackageError::Cancelled)
    ));
    assert!(
        !path.exists(),
        "ordinary cancellation removes only owned staging"
    );
    Ok(())
}

#[test]
fn preexisting_manifest_and_directory_are_left_intact() -> Check {
    let workspace = tempfile::tempdir()?;
    let path = workspace.path().join("bundle");
    let flag = AtomicBool::new(false);
    let context = PackageWriteContext::standalone(&|| false, &flag);
    let mut output = Output::create(&path, context)?;
    let prepared = manifest(&mut output)?;
    let foreign = path.join(wire::MANIFEST);
    fs::write(&foreign, b"foreign manifest")?;
    let publish = output.publish(&prepared);
    assert!(
        output.finish(publish).is_err(),
        "atomic no-clobber refuses a foreign commit leaf"
    );
    assert_eq!(
        fs::read(&foreign)?,
        b"foreign manifest",
        "foreign file survives cleanup"
    );
    assert!(
        Output::create(&path, context).is_err(),
        "even owned-looking existing path is refused"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn concurrent_namespace_replacement_is_retained() -> Check {
    let workspace = tempfile::tempdir()?;
    let old = workspace.path().join("moved");
    let replaced = workspace.path().join("replacement");
    let flag = AtomicBool::new(false);
    let context = PackageWriteContext::standalone(&|| false, &flag);
    let mut output = Output::create(&replaced, context)?;
    let replacement = manifest_for(&mut output, &replaced)?;
    fs::rename(&replaced, &old)?;
    fs::create_dir(&replaced)?;
    fs::write(replaced.join("foreign.txt"), b"foreign folder")?;
    let publish = output.publish(&replacement);
    assert!(
        output.finish(publish).is_err(),
        "old namespace was replaced"
    );
    assert_eq!(
        fs::read(replaced.join("foreign.txt"))?,
        b"foreign folder",
        "replacement contents survive"
    );
    assert!(
        old.join(wire::RECORDS).exists(),
        "uncertain moved output is retained"
    );
    Ok(())
}

fn manifest(output: &mut Output<'_>) -> Result<Manifest<'static>, PackageError> {
    let path = output.root.path().to_owned();
    manifest_for(output, &path)
}

fn manifest_for(output: &mut Output<'_>, path: &Path) -> Result<Manifest<'static>, PackageError> {
    let mut file = output.file(wire::RECORDS)?;
    file.write_all(b"sample")
        .map_err(|e| PackageError::write(path, e))?;
    let digest = output.complete_file(&path.join(wire::RECORDS), file)?;
    Ok(Manifest {
        format: wire::FORMAT,
        schema_version: wire::VERSION,
        bodies: &["male"],
        records: Stream {
            path: wire::RECORDS,
            digest,
            row_count: DecimalU64::new(1),
        },
        raw_artifacts: Totals {
            count: DecimalU64::new(0),
            byte_length: DecimalU64::new(0),
        },
    })
}
