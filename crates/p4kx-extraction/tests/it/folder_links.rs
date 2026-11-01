// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A folder of the target that changes during a run: a link is refused, a deleted one is made again.

use std::error::Error;
use std::fs;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_extraction::{Archive, ExtractOptions, Progress, extract};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

type TestResult = Result<(), Box<dyn Error>>;

#[cfg(unix)]
fn link(original: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(original, link).is_ok()
}

#[cfg(windows)]
fn link(original: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_dir(original, link).is_ok()
        || std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(original)
            .output()
            .is_ok_and(|output| output.status.success())
}

fn archive(dir: &Path) -> Result<Archive, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Dir\\one.txt", b"1".to_vec()))
        .entry(TestEntry::stored("Data\\Dir\\two.txt", b"2".to_vec()))
        .entry(TestEntry::stored("Data\\Dir\\three.txt", b"3".to_vec()))
        .write_to(&path)?;
    Ok(Archive::open_with_key(
        &path,
        Some(p4kx_testkit::test_key()),
    )?)
}

fn sequential() -> ExtractOptions<'static> {
    ExtractOptions {
        threads: NonZeroUsize::new(1),
        ..ExtractOptions::default()
    }
}

#[test]
fn a_folder_swapped_for_a_link_mid_run_is_refused_for_the_later_entries() -> TestResult {
    refuses_after_swap(false)
}

#[test]
fn a_parent_swapped_for_a_link_mid_run_is_refused_for_the_later_entries() -> TestResult {
    refuses_after_swap(true)
}

fn refuses_after_swap(parent: bool) -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let target = dir.path().join("out");
    let outside = dir.path().join("outside");
    fs::create_dir(&target)?;
    let (swapped_path, watched) = if parent {
        (target.join("Data"), outside.join("Dir"))
    } else {
        (target.join("Data").join("Dir"), outside.clone())
    };
    fs::create_dir_all(&watched)?;
    let probe = dir.path().join("probe");
    if !link(&outside, &probe) {
        if cfg!(unix) {
            return Err(
                "cannot create a symbolic link on Unix, so the test would check nothing".into(),
            );
        }
        return Ok(());
    }
    fs::remove_dir(&probe).or_else(|_| fs::remove_file(&probe))?;
    let swapped = AtomicBool::new(false);
    let on_progress = |progress: Progress| {
        if progress.entries_done >= 1 && !swapped.swap(true, Ordering::SeqCst) {
            let removed = fs::remove_dir_all(&swapped_path);
            assert!(removed.is_ok(), "remove the folder: {removed:?}");
            assert!(link(&outside, &swapped_path), "link in its place");
        }
    };
    let options = ExtractOptions {
        progress: Some(&on_progress),
        ..sequential()
    };
    let entries: Vec<_> = archive.entries().iter().collect();

    let report = extract(&archive, &entries, &target, &options);

    assert_eq!(report.extracted(), 1, "only the Entry before the swap");
    assert_eq!(report.failures().len(), 2, "the later Entries are refused");
    let leaked = fs::read_dir(&watched)?.count();
    assert_eq!(leaked, 0, "nothing is written through the link");
    Ok(())
}

#[test]
fn a_folder_deleted_mid_run_is_made_again() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let target = dir.path().join("out");
    fs::create_dir(&target)?;
    let deleted = AtomicBool::new(false);
    let on_progress = |progress: Progress| {
        if progress.entries_done >= 1 && !deleted.swap(true, Ordering::SeqCst) {
            fs::remove_dir_all(target.join("Data").join("Dir")).expect("remove the folder");
        }
    };
    let options = ExtractOptions {
        progress: Some(&on_progress),
        ..sequential()
    };
    let entries: Vec<_> = archive.entries().iter().collect();

    let report = extract(&archive, &entries, &target, &options);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(
        fs::read(target.join("Data").join("Dir").join("three.txt"))?,
        b"3",
        "the later Entries are written"
    );
    Ok(())
}
