// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crate::origin_file_io::{self, StageDisposition};
use crate::output_file::StagedOutput;

use super::{FreshPublication, try_create_fresh, try_create_with};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const PAYLOAD: &[u8] = b"staged content";

#[test]
fn a_clean_new_name_installs_its_bytes_with_a_foreign_lock_untouched() -> TestResult {
    let root = tempfile::tempdir()?;
    let target = root.path().join("source.bin");
    let foreign = root.path().join(".p4kx-origin.lock");
    fs::create_dir(&foreign)?;
    let sentinel = foreign.join("sentinel");
    fs::write(&sentinel, b"unrelated directory")?;

    let result = stage(&target, |staged| try_create_fresh(staged, None, || false))?;

    assert_eq!(
        result,
        FreshPublication::Installed,
        "the lock was not acquired"
    );
    assert_eq!(fs::read(target)?, PAYLOAD, "the exact bytes");
    assert_eq!(
        fs::read(sentinel)?,
        b"unrelated directory",
        "foreign object"
    );
    assert_eq!(
        fs::read_dir(root.path())?.count(),
        2,
        "only the original lock and payload"
    );
    Ok(())
}

#[test]
fn any_prior_binding_or_reserved_output_takes_the_locked_path() -> TestResult {
    for name in [
        "source.bin",
        "source.bin.p4kx-origin.jsonl",
        ".p4kx-origin.jsonl",
    ] {
        let root = tempfile::tempdir()?;
        let target = root.path().join("source.bin");
        let earlier = root.path().join(name);
        fs::write(&earlier, b"potential prior proof")?;
        let result = stage(&target, |staged| try_create_fresh(staged, None, || false))?;
        assert_eq!(
            result,
            FreshPublication::RequiresLock,
            "cannot bypass {name}"
        );
        assert_eq!(fs::read(earlier)?, b"potential prior proof", "unchanged");
    }
    let root = tempfile::tempdir()?;
    let target = root.path().join("source.bin.p4kx-origin.jsonl");
    let result = stage(&target, |staged| try_create_fresh(staged, None, || false))?;
    assert_eq!(
        result,
        FreshPublication::RequiresLock,
        "reserved output lane"
    );
    assert!(!target.exists(), "no suffix write");
    Ok(())
}

#[test]
fn a_lexical_parent_dir_uses_the_existing_guarded_path() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("bridge"))?;
    let target = root.path().join("bridge/../source.bin");
    let result = stage(&target, |staged| try_create_fresh(staged, None, || false))?;
    assert_eq!(
        result,
        FreshPublication::RequiresLock,
        "exact native path left to the guard"
    );
    assert!(!target.exists(), "no unguarded creation");
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_redirected_parent_uses_the_existing_guarded_path() -> TestResult {
    let root = tempfile::tempdir()?;
    let real = root.path().join("real");
    let alias = root.path().join("alias");
    fs::create_dir(&real)?;
    std::os::unix::fs::symlink(&real, &alias)?;
    let target = alias.join("source.bin");
    let result = stage(&target, |staged| try_create_fresh(staged, None, || false))?;
    assert_eq!(
        result,
        FreshPublication::RequiresLock,
        "no physical lease via a link"
    );
    assert!(!target.exists(), "no unguarded creation");
    Ok(())
}

#[test]
fn a_target_appearing_after_admission_is_not_overwritten() -> TestResult {
    let root = tempfile::tempdir()?;
    let target = root.path().join("source.bin");
    let result = stage(&target, |staged| {
        try_create_with(
            staged,
            None,
            || false,
            |staged, target| {
                fs::write(target, b"first writer's artifact")?;
                let sibling = target.with_extension("bin.p4kx-origin.jsonl");
                fs::write(sibling, b"first writer's proof")?;
                origin_file_io::install_without_replacing(staged, target)
            },
        )
    })?;
    assert_eq!(
        result,
        FreshPublication::Contended,
        "the atomic create lost"
    );
    assert_eq!(
        fs::read(&target)?,
        b"first writer's artifact",
        "first bytes"
    );
    assert_eq!(
        fs::read(target.with_extension("bin.p4kx-origin.jsonl"))?,
        b"first writer's proof",
        "the first writer's companion"
    );
    assert_eq!(fs::read_dir(root.path())?.count(), 2, "no pending output");
    Ok(())
}

fn stage(
    target: &Path,
    place: impl FnOnce(&StagedOutput<'_>) -> Result<FreshPublication, crate::ExtractionError>,
) -> io::Result<FreshPublication> {
    origin_file_io::write_staged(
        target,
        None,
        None,
        |writer| writer.write_all(PAYLOAD),
        |(), staged| {
            Ok(StageDisposition::Cleanup(
                place(staged).map_err(io::Error::other)?,
            ))
        },
    )
}
