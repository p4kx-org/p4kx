// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io;
use std::path::Path;

use p4kx_extraction::{
    InputScope, SourceInput, SourceLocation, SourceUseRole, SourceUses, read_observed_file,
    write_origin_companion,
};

use super::{FileReceipt, StdoutReceipt};

type TestResult = Result<(), Box<dyn Error>>;

const PAYLOAD: &[u8] = b"<same/>";

fn publish(output: &FileReceipt<'_>, source: &Path) -> TestResult {
    let input = read_observed_file(source)?;
    let uses = SourceUses::new();
    let scope = uses.observe(
        SourceInput::File(input.origin().ok_or("missing file origin")?),
        InputScope::Payload,
    )?;
    let datum = scope.at(
        SourceLocation::Metadata {
            field: "name".to_owned(),
        },
        SourceUseRole::Payload,
    )?;
    let origin = uses.origin(&[datum], true);
    let inventory = uses.freeze()?;
    output.publish(
        |writer| writer.write_all(PAYLOAD),
        |writer, binding, ()| {
            write_origin_companion(
                writer,
                binding,
                &inventory,
                [(
                    &p4kx_extraction::OriginOutputAddress::OpaquePayload,
                    &origin,
                )],
            )
        },
    )?;
    Ok(())
}

#[test]
fn equal_byte_replacement_invalidates_the_previous_origin_before_publication() -> TestResult {
    let dir = tempfile::tempdir()?;
    let previous = dir.path().join("source-before.xml");
    let current = dir.path().join("source-after.xml");
    fs::write(&previous, PAYLOAD)?;
    fs::write(&current, PAYLOAD)?;
    let artifact = dir.path().join("target.xml");
    let output = FileReceipt::new(&artifact, None, None)?;
    publish(&output, &previous)?;
    let old = fs::read_to_string(output.path())?;
    assert!(old.contains("source-before.xml"));

    let stage = output
        .publish(
            |writer| {
                writer.write_all(b"<incomplete")?;
                Err::<(), _>(io::Error::other("injected artifact failure"))
            },
            |_, _, ()| unreachable!("an incomplete artifact cannot produce a receipt"),
        )
        .expect_err("the staged artifact must fail");
    assert_eq!(stage.to_string(), "injected artifact failure");
    assert_eq!(fs::read(&artifact)?, PAYLOAD);
    assert_eq!(fs::read_to_string(output.path())?, old);

    let failure = output
        .publish(
            |writer| writer.write_all(PAYLOAD),
            |writer, _, ()| {
                assert!(!output.path().exists());
                writer.write_all(b"partial origin")?;
                Err(io::Error::other("injected receipt failure"))
            },
        )
        .expect_err("an incomplete receipt must fail");
    assert_eq!(failure.to_string(), "injected receipt failure");
    assert_eq!(fs::read(&artifact)?, PAYLOAD);
    assert!(!output.path().exists());

    publish(&output, &current)?;
    let new = fs::read_to_string(output.path())?;
    assert!(new.contains("source-after.xml"));
    assert_eq!(new.lines().next(), old.lines().next());
    assert_ne!(new, old);
    Ok(())
}

#[test]
fn contending_file_publication_cannot_invalidate_or_replace_the_previous_receipt() -> TestResult {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("source.xml");
    fs::write(&source, PAYLOAD)?;
    let first = FileReceipt::new(&dir.path().join("first.xml"), None, None)?;
    let second_path = dir.path().join("second.xml");
    let second = FileReceipt::new(&second_path, None, None)?;
    publish(&second, &source)?;
    let previous = fs::read(second.path())?;
    let empty = SourceUses::new().freeze()?;
    first.publish(
        |writer| writer.write_all(PAYLOAD),
        |writer, binding, ()| {
            let contention = second
                .publish(
                    |writer| writer.write_all(b"<contending/>"),
                    |_, _, ()| unreachable!("the contending writer cannot commit"),
                )
                .expect_err("the directory lock must reject overlapping publication");
            assert_eq!(contention.kind(), io::ErrorKind::WouldBlock, "{contention}");
            write_origin_companion(writer, binding, &empty, [])
        },
    )?;
    assert_eq!(fs::read(second_path)?, PAYLOAD);
    assert_eq!(fs::read(second.path())?, previous);
    assert!(first.path().is_file());
    publish(&second, &source)?;
    Ok(())
}

#[test]
fn arbitrary_receipt_destinations_cannot_replace_an_artifact_or_the_lock() -> TestResult {
    let dir = tempfile::tempdir()?;
    let lock = dir.path().join(".P4KX-ORIGIN.LOCK");
    assert!(StdoutReceipt::new(&lock, None).is_err());
    assert!(FileReceipt::new(&lock, None, None).is_err());
    assert!(!lock.exists());

    let source = dir.path().join("source.xml");
    fs::write(&source, PAYLOAD)?;
    let artifact = dir.path().join("artifact.xml");
    let output = FileReceipt::new(&artifact, None, None)?;
    publish(&output, &source)?;
    assert!(StdoutReceipt::new(output.path(), None).is_err());
    assert!(FileReceipt::new(output.path(), None, None).is_err());
    fs::remove_file(&artifact)?;
    assert!(StdoutReceipt::new(&artifact, None).is_err());
    assert!(!artifact.exists());

    fs::write(output.path(), b"unrelated file")?;
    assert!(publish(&output, &source).is_err());
    assert_eq!(fs::read(output.path())?, b"unrelated file");
    assert!(!artifact.exists());
    Ok(())
}
