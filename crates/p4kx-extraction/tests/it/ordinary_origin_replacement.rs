// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::OnceLock;

use p4kx_extraction::{
    Archive, ConflictPolicy, ExtractOptions, ExtractionError, ExtractionReport, FileReceipt,
    InputScope, OriginOutputAddress, Progress, SourceInput, SourceLocation, SourceUseRole,
    SourceUses, extract, write_origin_companion,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

type TestResult = Result<(), Box<dyn Error>>;

const ENTRY: &str = "Data/panel.bin";
const LOCK_NAME: &str = ".p4kx-origin.lock";
const PAYLOAD: &[u8] = b"same fixture contents";

fn fixture(path: &Path, entry: &str) -> Result<Archive, Box<dyn Error>> {
    ArchiveBuilder::new()
        .entry(TestEntry::stored(entry, PAYLOAD.to_vec()))
        .write_to(path)?;
    Ok(Archive::open(path)?)
}

fn run(archive: &Archive, output: &Path, policy: ConflictPolicy) -> ExtractionReport {
    let entries: Vec<_> = archive.entries().iter().collect();
    extract(
        archive,
        &entries,
        output,
        &ExtractOptions {
            policy,
            ..ExtractOptions::default()
        },
    )
}

#[test]
fn keep_decisions_bypass_a_foreign_lock_directory_and_overwrite_reports_it() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = fixture(&scratch.path().join("fixture.p4k"), ENTRY)?;
    let output = scratch.path().join("out");
    let artifact = output.join(ENTRY);
    let lock = output.join("Data").join(LOCK_NAME);
    let sentinel = lock.join("sentinel");
    fs::create_dir_all(&lock)?;
    fs::write(&sentinel, b"foreign directory contents")?;
    let created = run(&archive, &output, ConflictPolicy::IfChanged);
    assert!(
        created.is_complete(),
        "the clean new file needs no lock: {created:?}"
    );
    assert_eq!(fs::read(&artifact)?, PAYLOAD, "new original bytes");

    for policy in [ConflictPolicy::Skip, ConflictPolicy::IfChanged] {
        let report = run(&archive, &output, policy);
        assert!(report.is_complete(), "keep bypasses the lock: {report:?}");
        assert_eq!(report.skipped().len(), 1, "the output was kept");
    }
    fs::write(&artifact, b"previous output bytes")?;
    let report = run(&archive, &output, ConflictPolicy::Overwrite);
    let [failure] = report.failures() else {
        return Err(format!("expected one lock collision: {report:?}").into());
    };
    assert!(
        matches!(
            failure.error(),
            ExtractionError::OriginLockCollision { path }
                if path == &fs::canonicalize(&lock)?
        ),
        "the reserved lock has a typed collision: {failure:?}"
    );
    assert_eq!(
        fs::read(artifact)?,
        b"previous output bytes",
        "old artifact"
    );
    assert_eq!(
        fs::read(sentinel)?,
        b"foreign directory contents",
        "foreign tree"
    );
    assert_eq!(
        fs::read_dir(output.join("Data"))?.count(),
        2,
        "only the prior artifact and foreign directory remain"
    );
    Ok(())
}

#[test]
fn native_companion_suffix_is_usable_when_no_owned_receipt_occupies_it() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let literal = "Data/panel.bin.p4kx-origin.jsonl";
    let archive = fixture(&scratch.path().join("literal.p4k"), literal)?;
    let output = scratch.path().join("out");
    fs::create_dir_all(&output)?;
    let report = run(&archive, &output, ConflictPolicy::Overwrite);
    assert!(report.is_complete(), "plain native suffix: {report:?}");
    assert_eq!(
        fs::read(output.join(literal))?,
        PAYLOAD,
        "native filename and bytes"
    );

    fs::remove_file(output.join(literal))?;
    let source = fixture(&scratch.path().join("prior.p4k"), ENTRY)?;
    let artifact = output.join(ENTRY);
    publish_old_receipt(&source, &artifact)?;
    let prior = fs::read(output.join(literal))?;
    let report = run(&archive, &output, ConflictPolicy::Overwrite);
    assert_eq!(report.failures().len(), 1, "owned companion collision");
    assert_eq!(
        fs::read(output.join(literal))?,
        prior,
        "owned proof preserved"
    );
    assert_eq!(
        fs::read(artifact)?,
        PAYLOAD,
        "original owner bytes preserved"
    );
    Ok(())
}

#[test]
fn identical_bytes_from_another_archive_retire_the_old_proof_only_on_overwrite() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let original = fixture(&scratch.path().join("first.p4k"), ENTRY)?;
    let replacement = fixture(&scratch.path().join("second.p4k"), ENTRY)?;
    let output = scratch.path().join("out");
    let artifact = output.join(ENTRY);
    fs::create_dir_all(output.join("Data"))?;
    publish_old_receipt(&original, &artifact)?;

    let canonical = FileReceipt::new(&artifact, None, None)?;
    let prior = fs::read(canonical.path())?;
    for policy in [ConflictPolicy::Skip, ConflictPolicy::IfChanged] {
        let report = run(&replacement, &output, policy);
        assert!(report.is_complete(), "keep: {report:?}");
        assert_eq!(report.skipped().len(), 1, "keep outcome");
        assert_eq!(fs::read(canonical.path())?, prior, "existing source proof");
    }
    let report = run(&replacement, &output, ConflictPolicy::Overwrite);
    assert!(report.is_complete(), "ordinary overwrite: {report:?}");
    assert_eq!(fs::read(artifact)?, PAYLOAD, "byte-identical replacement");
    assert!(
        !canonical.path().exists(),
        "the old identity claim is retired"
    );
    Ok(())
}

#[test]
fn nonempty_regular_lock_preserves_its_bytes_and_the_old_artifact() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = fixture(&scratch.path().join("fixture.p4k"), ENTRY)?;
    let output = scratch.path().join("out");
    let artifact = output.join(ENTRY);
    fs::create_dir_all(output.join("Data"))?;
    let lock = output.join("Data").join(LOCK_NAME);
    fs::write(&lock, b"foreign regular file")?;
    fs::write(&artifact, b"previous bytes")?;
    let report = run(&archive, &output, ConflictPolicy::Overwrite);
    assert_eq!(report.failures().len(), 1, "the existing empty-lock rule");
    assert_eq!(fs::read(lock)?, b"foreign regular file", "foreign bytes");
    assert_eq!(fs::read(artifact)?, b"previous bytes", "old artifact");
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_redirected_lock_refuses_replacement_without_touching_its_referent() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = fixture(&scratch.path().join("fixture.p4k"), ENTRY)?;
    let output = scratch.path().join("out");
    let artifact = output.join(ENTRY);
    let lock = output.join("Data").join(LOCK_NAME);
    let referent = scratch.path().join("unrelated");
    fs::create_dir_all(output.join("Data"))?;
    fs::write(&referent, b"foreign referent")?;
    std::os::unix::fs::symlink(&referent, &lock)?;
    fs::write(&artifact, b"previous bytes")?;

    let report = run(&archive, &output, ConflictPolicy::Overwrite);
    let [failure] = report.failures() else {
        return Err(format!("expected one redirected lock collision: {report:?}").into());
    };
    assert!(
        matches!(failure.error(), ExtractionError::OriginLockCollision { .. }),
        "the symlink is a typed lock collision: {failure:?}"
    );
    assert_eq!(fs::read_link(lock)?, referent, "the symlink remains");
    assert_eq!(
        fs::read(referent)?,
        b"foreign referent",
        "the referent remains"
    );
    assert_eq!(
        fs::read(artifact)?,
        b"previous bytes",
        "the old output remains"
    );
    Ok(())
}

#[test]
fn same_parent_ordinary_replacements_share_the_ephemeral_lock_protocol() -> TestResult {
    const FILES: usize = 24;
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("same-parent.p4k");
    let mut builder = ArchiveBuilder::new();
    for index in 0..FILES {
        builder = builder.entry(TestEntry::stored(
            format!("Data/file{index}.bin"),
            PAYLOAD.to_vec(),
        ));
    }
    builder.write_to(&path)?;
    let archive = Archive::open(&path)?;
    let output = scratch.path().join("out");
    fs::create_dir_all(&output)?;

    fs::create_dir_all(output.join("Data"))?;
    for index in 0..FILES {
        fs::write(output.join(format!("Data/file{index}.bin")), b"old bytes")?;
    }
    let report = run(&archive, &output, ConflictPolicy::Overwrite);
    assert!(
        report.is_complete(),
        "each existing artifact was replaced: {report:?}"
    );
    assert_eq!(
        fs::read_dir(output.join("Data"))?.count(),
        FILES,
        "the lock generations are retired and all files remain"
    );
    for index in 0..FILES {
        assert_eq!(
            fs::read(output.join(format!("Data/file{index}.bin")))?,
            PAYLOAD,
            "original bytes for file {index}"
        );
    }
    Ok(())
}

#[test]
fn an_orphaned_owned_companion_requires_the_locked_retirement_path() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let source = fixture(&scratch.path().join("first.p4k"), ENTRY)?;
    let replacement = fixture(&scratch.path().join("second.p4k"), ENTRY)?;
    let output = scratch.path().join("out");
    let artifact = output.join(ENTRY);
    fs::create_dir_all(output.join("Data"))?;
    publish_old_receipt(&source, &artifact)?;
    let receipt = FileReceipt::new(&artifact, None, None)?;
    fs::remove_file(&artifact)?;
    assert!(receipt.path().is_file(), "the genuine old proof remains");

    let report = run(&replacement, &output, ConflictPolicy::Overwrite);
    assert!(report.is_complete(), "locked reinstallation: {report:?}");
    assert_eq!(
        fs::read(artifact)?,
        PAYLOAD,
        "the byte-identical new artifact"
    );
    assert!(
        !receipt.path().exists(),
        "the orphaned past proof was retired"
    );
    Ok(())
}

#[test]
fn skip_keeps_an_artifact_and_proof_published_during_payload_staging() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let first = fixture(&scratch.path().join("first.p4k"), ENTRY)?;
    let second = fixture(&scratch.path().join("second.p4k"), ENTRY)?;
    let output = scratch.path().join("out");
    fs::create_dir_all(output.join("Data"))?;
    let artifact = output.join(ENTRY);
    let receipt = FileReceipt::new(&artifact, None, None)?;
    let original_proof = OnceLock::new();
    let progress = |current: Progress| {
        if current.bytes_done != 0 && current.entries_done == 0 {
            let _ = original_proof.get_or_init(|| {
                publish_old_receipt(&first, &artifact)
                    .map_err(|error| io::Error::other(error.to_string()))?;
                fs::read(receipt.path())
            });
        }
    };
    let entry = second.entries().first().ok_or("one fixture entry")?;
    let report = extract(
        &second,
        &[entry],
        &output,
        &ExtractOptions {
            policy: ConflictPolicy::Skip,
            progress: Some(&progress),
            ..ExtractOptions::default()
        },
    );

    let original_proof = original_proof
        .into_inner()
        .ok_or("the first publisher ran")??;
    assert!(
        report.is_complete(),
        "the raced destination is kept: {report:?}"
    );
    assert_eq!(report.skipped().len(), 1, "one kept incumbent");
    assert_eq!(
        report.extracted(),
        0,
        "the staged payload was not installed"
    );
    assert_eq!(
        fs::read(receipt.path())?,
        original_proof,
        "the earlier proof remains"
    );
    assert_eq!(fs::read(artifact)?, PAYLOAD, "the earlier bytes remain");
    Ok(())
}

fn publish_old_receipt(archive: &Archive, output: &Path) -> TestResult {
    let entry = archive.entries().first().ok_or("one fixture entry")?;
    let observed = archive
        .observed_entry(entry)
        .ok_or("fixture archive observation")?;
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::ArchiveView(observed), InputScope::Payload)?;
    let used = scope.at(
        SourceLocation::Subresource {
            namespace: "archiveEntryDecodedBytes".into(),
            indices: vec![0, entry.uncompressed_size()],
            field: "bytes".into(),
        },
        SourceUseRole::Payload,
    )?;
    let inventory = sources.freeze()?;
    let origin = sources.origin(&[used], true);
    let receipt = FileReceipt::new(output, None, None)?;
    receipt.publish(
        |writer| {
            let mut reader = archive.open_entry(entry).map_err(io::Error::other)?;
            io::copy(&mut reader, writer)?;
            writer.flush()
        },
        |writer, binding, ()| {
            write_origin_companion(
                writer,
                binding,
                &inventory,
                [(&OriginOutputAddress::OpaquePayload, &origin)],
            )
        },
    )?;
    Ok(())
}
