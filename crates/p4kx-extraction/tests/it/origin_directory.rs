// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;
use std::thread;

use p4kx_extraction::{
    ConversionError, DirectoryCommit, DirectoryReceipt, FileReceipt, OriginCompanionRow,
    OriginOutputAddress, SourceUses, write_origin_companion,
};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const MARKER: &str = ".p4kx-origin.jsonl";

fn commit<'owner>(
    owner: &'owner DirectoryReceipt<'_>,
    name: &str,
    bytes: &[u8],
) -> Result<DirectoryCommit<'owner>, ConversionError> {
    let ((), receipt) = owner.write(OsStr::new(name), |writer| {
        writer
            .write_all(bytes)
            .map_err(|source| ConversionError::Write {
                path: name.into(),
                source,
            })
    })?;
    Ok(receipt)
}

fn publish<'owner>(
    owner: &'owner DirectoryReceipt<'_>,
    commits: Vec<DirectoryCommit<'owner>>,
) -> TestResult<bool> {
    let uses = SourceUses::new();
    let count = commits.len();
    Ok(owner.publish(
        commits,
        &uses.freeze()?,
        (0..count).map(|artifact| {
            (
                artifact,
                OriginOutputAddress::OpaquePayload,
                uses.origin(&[], true),
            )
        }),
    )?)
}

fn rows(folder: &Path) -> TestResult<Vec<Value>> {
    Ok(fs::read_to_string(folder.join(MARKER))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?)
}

#[test]
fn parallel_artifacts_share_one_owner_and_bind_exact_committed_bytes() -> TestResult {
    let folder = tempfile::tempdir()?;
    let owner = DirectoryReceipt::new(folder.path(), None)?;
    let (left, right) = thread::scope(|scope| {
        let first = scope.spawn(|| commit(&owner, "first.bin", b"abc"));
        let second = scope.spawn(|| commit(&owner, "second.bin", b"other"));
        (
            first.join().expect("first artifact thread"),
            second.join().expect("second artifact thread"),
        )
    });
    let (left, right) = (left?, right?);
    assert_eq!(left.byte_length(), 3);
    assert_eq!(right.byte_length(), 5);
    assert!(publish(&owner, vec![left, right])?);

    let rows = rows(folder.path())?;
    let schema = schemars::schema_for!(OriginCompanionRow<'static>).to_value();
    let validator = jsonschema::validator_for(&schema)?;
    for row in &rows {
        validator.validate(row).map_err(|error| error.to_string())?;
    }
    assert_eq!(rows[0]["binding"]["type"], "directory");
    assert_eq!(rows[1]["path"]["value"], "first.bin");
    assert_eq!(rows[1]["byteLength"], "3");
    assert_eq!(
        rows[1]["sha256"],
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(rows[2]["path"]["value"], "second.bin");
    assert_eq!(rows[3]["type"], "artifactTableEnd");
    assert_eq!(rows[3]["artifacts"], "2");
    for (ordinal, row) in rows[4..6].iter().enumerate() {
        assert_eq!(row["type"], "artifactField");
        assert_eq!(row["artifact"], ordinal.to_string());
        assert_eq!(row["containsEncryptedSource"], false);
        assert_eq!(row["sourceSetComplete"], true);
    }
    assert_eq!(rows[6]["type"], "end");
    assert_eq!(rows[6]["fields"], "2");
    assert!(commit(&owner, "too-late.bin", b"late").is_err());
    Ok(())
}

fn fail_before_replace(owner: &DirectoryReceipt<'_>, name: &str) {
    let result = owner.write(OsStr::new(name), |_| {
        Err::<(), _>(ConversionError::Write {
            path: name.into(),
            source: io::Error::other("staged artifact failed"),
        })
    });
    assert!(result.is_err(), "the failed write is never committed");
}

#[test]
fn a_partial_run_proves_only_replaced_outputs_and_preserves_old_proof_until_replacement()
-> TestResult {
    let folder = tempfile::tempdir()?;
    let marker = folder.path().join(MARKER);
    {
        let owner = DirectoryReceipt::new(folder.path(), None)?;
        let left = commit(&owner, "first.bin", b"same bytes")?;
        let right = commit(&owner, "second.bin", b"old bytes")?;
        assert!(publish(&owner, vec![left, right])?);
    }
    let prior = fs::read(&marker)?;
    {
        let owner = DirectoryReceipt::new(folder.path(), None)?;
        fail_before_replace(&owner, "first.bin");
        assert!(!publish(&owner, Vec::new())?);
        assert_eq!(fs::read(&marker)?, prior);
    }
    let owner = DirectoryReceipt::new(folder.path(), None)?;
    let left = commit(&owner, "first.bin", b"same bytes")?;
    assert!(!marker.exists(), "even equal bytes cannot keep old proof");
    fail_before_replace(&owner, "second.bin");
    assert!(publish(&owner, vec![left])?);
    let rows = rows(folder.path())?;
    assert_eq!(rows[1]["path"]["value"], "first.bin");
    assert_eq!(rows[2]["artifacts"], "1");
    assert_eq!(fs::read(folder.path().join("second.bin"))?, b"old bytes");
    Ok(())
}

fn publish_file(path: &Path) -> TestResult {
    let receipt = FileReceipt::new(path, None, None)?;
    let empty = SourceUses::new().freeze()?;
    receipt.publish(
        |writer| writer.write_all(b"same"),
        |writer, binding, ()| write_origin_companion(writer, binding, &empty, []),
    )?;
    Ok(())
}

#[test]
fn file_and_directory_writers_invalidate_only_conflicting_proofs_before_replacement() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let marker = folder.path().join(MARKER);
    let changed = folder.path().join("artifact.bin");
    let unrelated = folder.path().join("unrelated.bin");
    {
        let owner = DirectoryReceipt::new(folder.path(), None)?;
        assert!(publish(
            &owner,
            vec![commit(&owner, "artifact.bin", b"same")?]
        )?);
        assert!(
            publish_file(&unrelated).is_err(),
            "the shared directory lock is held"
        );
    }
    let old = fs::read(&marker)?;
    publish_file(&unrelated)?;
    assert_eq!(fs::read(&marker)?, old, "disjoint aggregate proof remains");
    publish_file(&changed)?;
    assert!(!marker.exists(), "the aggregate covered the replaced file");
    let file_receipt = FileReceipt::new(&changed, None, None)?;
    assert!(file_receipt.path().exists());

    let owner = DirectoryReceipt::new(folder.path(), None)?;
    let changed = commit(&owner, "artifact.bin", b"same")?;
    assert!(
        !file_receipt.path().exists(),
        "the sibling proof covered the replaced file"
    );
    let failure = owner.publish([changed], &SourceUses::new().freeze()?, []);
    assert!(
        failure.is_err(),
        "a committed artifact needs its addressed fields"
    );
    assert!(!marker.exists());
    assert_eq!(fs::read(folder.path().join("artifact.bin"))?, b"same");
    Ok(())
}

#[test]
fn foreign_markers_and_unreported_commits_cannot_certify_or_replace_another_output() -> TestResult {
    let folder = tempfile::tempdir()?;
    let marker = folder.path().join(MARKER);
    fs::write(&marker, b"foreign")?;
    assert!(DirectoryReceipt::new(folder.path(), None).is_err());
    assert!(publish_file(&folder.path().join("first.bin")).is_err());
    assert_eq!(fs::read(&marker)?, b"foreign");
    fs::remove_file(&marker)?;

    let owner = DirectoryReceipt::new(folder.path(), None)?;
    let committed = commit(&owner, "first.bin", b"data")?;
    assert!(commit(&owner, "first.bin", b"duplicate").is_err());
    assert!(publish(&owner, Vec::new()).is_err());
    assert!(!marker.exists());
    assert!(publish(&owner, vec![committed])?);
    assert_eq!(fs::read(folder.path().join("first.bin"))?, b"data");
    Ok(())
}

#[test]
fn a_case_alias_that_appears_while_staged_cannot_be_overwritten() -> TestResult {
    let folder = tempfile::tempdir()?;
    let probe = folder.path().join("CaseProbe");
    fs::write(&probe, b"probe")?;
    let folds_case = folder.path().join("caseprobe").exists();
    let other = folder.path().join("image.png");
    let owner = DirectoryReceipt::new(folder.path(), None)?;
    let staged = owner.write(OsStr::new("Image.png"), |writer| {
        writer
            .write_all(b"staged")
            .map_err(|source| ConversionError::Write {
                path: "Image.png".into(),
                source,
            })?;
        fs::write(&other, b"appeared").map_err(|source| ConversionError::Write {
            path: other.clone(),
            source,
        })
    });
    if folds_case {
        assert!(staged.is_err(), "the alias occupies the reserved target");
        assert!(!publish(&owner, Vec::new())?);
    } else {
        let ((), commit) = staged?;
        assert!(publish(&owner, vec![commit])?);
        assert_eq!(fs::read(folder.path().join("Image.png"))?, b"staged");
    }
    assert_eq!(fs::read(&other)?, b"appeared");
    Ok(())
}

#[test]
fn an_existing_artifact_replaced_during_staging_keeps_its_new_identity() -> TestResult {
    let folder = tempfile::tempdir()?;
    let target = folder.path().join("artifact.bin");
    fs::write(&target, b"original")?;
    let owner = DirectoryReceipt::new(folder.path(), None)?;
    let staged = owner.write(OsStr::new("artifact.bin"), |writer| {
        let write_error = |source| ConversionError::Write {
            path: target.clone(),
            source,
        };
        writer.write_all(b"staged").map_err(write_error)?;
        fs::rename(&target, folder.path().join("retained.bin")).map_err(write_error)?;
        fs::write(&target, b"replacement").map_err(write_error)
    });
    assert!(
        staged.is_err(),
        "the actual target no longer has its reserved identity"
    );
    assert_eq!(fs::read(&target)?, b"replacement");
    assert!(!publish(&owner, Vec::new())?);
    Ok(())
}

#[test]
fn concurrent_case_spellings_follow_the_destination_filesystem() -> TestResult {
    let folder = tempfile::tempdir()?;
    let probe = folder.path().join("CaseProbe");
    fs::write(&probe, b"probe")?;
    let folds_case = folder.path().join("caseprobe").exists();
    let owner = DirectoryReceipt::new(folder.path(), None)?;
    let (first, second) = thread::scope(|scope| {
        let left = scope.spawn(|| commit(&owner, "Name.png", b"first"));
        let right = scope.spawn(|| commit(&owner, "name.png", b"second"));
        (
            left.join().expect("first spelling worker"),
            right.join().expect("second spelling worker"),
        )
    });
    let commits = <[_; 2]>::from((first, second))
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if folds_case {
        assert_eq!(commits.len(), 1);
        assert!(commit(&owner, "NAME.png", b"third").is_err());
    } else {
        assert_eq!(commits.len(), 2);
        assert_eq!(fs::read(folder.path().join("Name.png"))?, b"first");
        assert_eq!(fs::read(folder.path().join("name.png"))?, b"second");
    }
    assert!(publish(&owner, commits)?);
    Ok(())
}
