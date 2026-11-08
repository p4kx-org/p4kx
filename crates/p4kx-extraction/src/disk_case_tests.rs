// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Read};

use super::{CachedIoError, CaseIndex, Listing, collect_names};

#[test]
fn a_name_with_other_case_finds_the_file_by_its_real_name() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    fs::create_dir(dir.path().join("Textures"))?;
    let real = dir.path().join("Textures").join("Rock_Diff.dds");
    fs::write(&real, b"DDS ")?;
    let asked = dir.path().join("textures").join("ROCK_diff.DDS");
    let mut index = CaseIndex::default();

    let found = index.variant(&asked)?.ok_or(io::ErrorKind::NotFound)?;

    assert_eq!(fs::read(&found)?, b"DDS ");
    if cfg!(target_os = "linux") {
        assert_eq!(found, real);
    }
    let (mut file, opened) = index.open_variant(&asked)?.ok_or(io::ErrorKind::NotFound)?;
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    assert_eq!(text, "DDS ");
    assert_eq!(opened, found);
    Ok(())
}

#[test]
fn an_exact_name_does_not_list_its_folder() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let real = dir.path().join("a.txt");
    fs::write(&real, b"x")?;
    let mut index = CaseIndex::default();

    assert_eq!(index.variant(&real)?.as_deref(), Some(real.as_path()));
    assert!(index.folders.is_empty());
    Ok(())
}

#[test]
fn a_missing_name_or_folder_finds_nothing() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    fs::write(dir.path().join("a.txt"), b"x")?;
    let mut index = CaseIndex::default();

    assert_eq!(index.variant(&dir.path().join("b.txt"))?, None);
    assert_eq!(index.variant(&dir.path().join("nope/a.txt"))?, None);
    assert!(index.open_variant(&dir.path().join("B.TXT"))?.is_none());
    Ok(())
}

#[test]
fn a_listing_is_read_once_and_exact_new_names_still_win() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    fs::write(dir.path().join("Late.txt"), b"x")?;
    let mut index = CaseIndex::default();
    assert!(index.variant(&dir.path().join("late.TXT"))?.is_some());
    let fresh = dir.path().join("Newer.txt");
    fs::write(&fresh, b"x")?;

    let seen = index.variant(&dir.path().join("newer.txt"))?;

    assert!(seen.is_none() || cfg!(windows), "{seen:?}");
    assert!(index.variant(&fresh)?.is_some());
    assert_eq!(index.folders.len(), usize::from(!cfg!(windows)));
    Ok(())
}

#[test]
fn two_names_differing_only_in_case_are_ambiguous() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let exact = dir.path().join("Same.txt");
    fs::write(&exact, b"1")?;
    fs::write(dir.path().join("SAME.TXT"), b"2")?;
    if fs::read_dir(dir.path())?.count() < 2 {
        return Ok(());
    }
    let mut index = CaseIndex::default();

    assert_eq!(index.variant(&dir.path().join("same.txt"))?, None);
    assert_eq!(index.variant(&exact)?, Some(exact));
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn an_exact_directory_is_not_replaced_to_find_a_later_component() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    fs::create_dir(dir.path().join("Textures"))?;
    fs::create_dir(dir.path().join("textures"))?;
    fs::write(dir.path().join("textures/x.dds"), b"x")?;
    let mut index = CaseIndex::default();

    assert_eq!(index.variant(&dir.path().join("Textures/X.DDS"))?, None);
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_file_in_the_directory_position_retains_the_os_error() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let parent = dir.path().join("a.txt");
    fs::write(&parent, b"x")?;
    let asked = parent.join("child");
    let expected = fs::symlink_metadata(&asked).expect_err("file is not a directory");
    let mut index = CaseIndex::default();

    let found = index.variant(&asked).expect_err("retain metadata failure");

    assert_eq!(found.kind(), io::ErrorKind::NotADirectory);
    assert_eq!(found.raw_os_error(), expected.raw_os_error());
    assert!(index.folders.is_empty());
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn a_broken_exact_symlink_does_not_choose_another_case() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let exact = dir.path().join("A.dds");
    std::os::unix::fs::symlink("absent", &exact)?;
    fs::write(dir.path().join("a.DDS"), b"x")?;
    let mut index = CaseIndex::default();

    assert_eq!(index.variant(&exact)?, Some(exact.clone()));
    assert!(index.open_variant(&exact)?.is_none());
    assert!(index.folders.is_empty());
    Ok(())
}

#[test]
fn cached_os_failures_retain_the_raw_code_and_kind() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let original = fs::read_dir(dir.path().join("absent")).expect_err("missing folder");
    let expected_code = original.raw_os_error();
    let expected_kind = original.kind();
    let cached = CachedIoError::new(original);

    for _ in 0..2 {
        let replay = cached.replay();
        assert_eq!(replay.kind(), expected_kind);
        assert_eq!(replay.raw_os_error(), expected_code);
    }
    Ok(())
}

#[test]
fn a_real_failed_listing_is_kept_after_the_folder_is_replaced() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let folder = dir.path().join("blocked");
    fs::write(&folder, b"file in directory position")?;
    let expected = fs::read_dir(&folder).expect_err("file is not a directory");
    let mut index = CaseIndex::default();
    let name = std::ffi::OsStr::new("Child.dds");
    let first = index
        .unique_match(&folder, name)
        .expect_err("listing failure");
    assert_eq!(first.raw_os_error(), expected.raw_os_error());
    fs::remove_file(&folder)?;
    fs::create_dir(&folder)?;
    fs::write(folder.join(name), b"now present")?;

    let repeated = index
        .unique_match(&folder, name)
        .expect_err("cached failure");

    assert_eq!(repeated.kind(), expected.kind());
    assert_eq!(repeated.raw_os_error(), expected.raw_os_error());
    assert_eq!(index.folders.len(), 1);
    Ok(())
}

#[derive(Debug)]
struct Denied;

impl fmt::Display for Denied {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("listing denied")
    }
}

impl Error for Denied {}

#[test]
fn a_partial_listing_is_failed_instead_of_used_for_a_match() {
    let listing = collect_names([
        Ok(std::ffi::OsString::from("Candidate.dds")),
        Err(io::Error::new(io::ErrorKind::PermissionDenied, Denied)),
    ]);

    let Listing::Failed(error) = listing else {
        panic!("a partial listing is not complete: {listing:?}");
    };
    assert_eq!(error.replay().kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn a_cached_custom_failure_preserves_its_original_typed_cause() {
    let cached = CachedIoError::new(io::Error::new(io::ErrorKind::PermissionDenied, Denied));

    for _ in 0..2 {
        let replay = cached.replay();
        assert_eq!(replay.kind(), io::ErrorKind::PermissionDenied);
        let wrapper = replay
            .get_ref()
            .and_then(|cause| cause.downcast_ref::<CachedIoError>())
            .expect("cached error wrapper");
        let original = wrapper
            .source()
            .and_then(|cause| cause.downcast_ref::<io::Error>())
            .expect("original I/O error");
        assert!(
            original
                .get_ref()
                .expect("original custom cause")
                .is::<Denied>()
        );
    }
}

#[test]
fn a_failed_listing_is_reused_but_an_exact_hit_ignores_it() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let exact = dir.path().join("File.dds");
    fs::write(&exact, b"x")?;
    let mut index = CaseIndex::default();
    index.folders.insert(
        dir.path().to_path_buf(),
        Listing::Failed(CachedIoError::new(io::Error::new(
            io::ErrorKind::PermissionDenied,
            Denied,
        ))),
    );

    for _ in 0..2 {
        let failure = index
            .unique_match(dir.path(), std::ffi::OsStr::new("FILE.DDS"))
            .expect_err("failed listing");
        assert_eq!(failure.kind(), io::ErrorKind::PermissionDenied);
    }
    assert_eq!(index.variant(&exact)?, Some(exact));
    assert_eq!(index.folders.len(), 1);
    Ok(())
}

#[cfg(unix)]
#[test]
fn lossy_keys_never_make_distinct_non_ascii_names_equal() -> io::Result<()> {
    use std::os::unix::ffi::OsStringExt;

    let dir = tempfile::tempdir()?;
    let actual = std::ffi::OsString::from_vec(vec![b'a', 0xff]);
    let asked = std::ffi::OsString::from_vec(vec![b'A', 0xfe]);
    fs::write(dir.path().join(actual), b"x")?;
    let mut index = CaseIndex::default();

    assert_eq!(index.variant(&dir.path().join(asked))?, None);
    Ok(())
}
