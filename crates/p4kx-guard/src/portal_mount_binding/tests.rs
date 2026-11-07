// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use super::MountBinding;
use crate::portal_alias::{AliasUncertainty, UnknownAlias};
use crate::portal_mount;

type TestResult = Result<(), Box<dyn Error>>;

struct Fixture {
    root: tempfile::TempDir,
    declared: PathBuf,
    first: PathBuf,
    second: PathBuf,
    ordinary: PathBuf,
    table: Vec<u8>,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let root = tempfile::tempdir()?;
        let first = root.path().join("visible-first");
        let second = root.path().join("visible-second");
        let ordinary = root.path().join("ordinary");
        for path in [&first, &second, &ordinary] {
            fs::create_dir(path)?;
        }
        let declared = root.path().join("runtime-doc");
        symlink(&first, &declared)?;
        let mut table = b"21 1 8:1 / / rw - ext4 /dev/root rw\n".to_vec();
        table.extend(mount_line(42, &first));
        table.extend(mount_line(43, &second));
        Ok(Self {
            root,
            declared,
            first,
            second,
            ordinary,
            table,
        })
    }

    fn raw(&self) -> Vec<u8> {
        bytestring(&self.declared)
    }
}

fn bytestring(path: &Path) -> Vec<u8> {
    let mut bytes = path.as_os_str().as_bytes().to_vec();
    bytes.push(0);
    bytes
}

fn mount_line(id: u32, path: &Path) -> Vec<u8> {
    let mut line = format!("{id} 21 0:{id} / ").into_bytes();
    for byte in path.as_os_str().as_bytes() {
        match byte {
            b' ' => line.extend_from_slice(b"\\040"),
            b'\t' => line.extend_from_slice(b"\\011"),
            b'\n' => line.extend_from_slice(b"\\012"),
            b'\\' => line.extend_from_slice(b"\\134"),
            byte => line.push(*byte),
        }
    }
    line.extend_from_slice(b" rw - fuse.portal portal rw\n");
    line
}

#[test]
fn an_authenticated_runtime_alias_binds_the_visible_mount_and_keeps_the_raw_spelling() -> TestResult
{
    let fixture = Fixture::new()?;
    let mut inventories = 0;
    let binding = MountBinding::capture(&fixture.raw(), &AtomicBool::new(false), || {
        inventories += 1;
        Ok(fixture.table.clone())
    })?;
    let mount = portal_mount::covering_mount(&fixture.table, fixture.first.as_os_str().as_bytes())?
        .ok_or("visible mount missing")?;
    binding.check_mount(&mount)?;
    binding.check_declared(&fixture.raw())?;
    assert_eq!(
        inventories, 2,
        "both namespace observations use fresh mount inventories"
    );
    assert_eq!(
        binding
            .check_declared(&bytestring(&fixture.first))
            .unwrap_err()
            .reason(),
        AliasUncertainty::UntrustedMount,
        "the original authenticated spelling is not replaced by derived canonical bytes"
    );
    Ok(())
}

#[test]
fn retargeting_the_authenticated_alias_or_replacing_its_mount_is_unknown() -> TestResult {
    for change in 0..3 {
        let fixture = Fixture::new()?;
        let mut inventories = 0;
        let result = MountBinding::capture(&fixture.raw(), &AtomicBool::new(false), || {
            inventories += 1;
            if inventories == 2 {
                if change < 2 {
                    fs::remove_file(&fixture.declared).map_err(|error| {
                        UnknownAlias::with_source(AliasUncertainty::Filesystem, error)
                    })?;
                    let target = if change == 0 {
                        &fixture.second
                    } else {
                        &fixture.ordinary
                    };
                    symlink(target, &fixture.declared).map_err(|error| {
                        UnknownAlias::with_source(AliasUncertainty::Filesystem, error)
                    })?;
                } else {
                    let mut changed = b"21 1 8:1 / / rw - ext4 /dev/root rw\n".to_vec();
                    changed.extend(mount_line(44, &fixture.first));
                    return Ok(changed);
                }
            }
            Ok(fixture.table.clone())
        });
        assert_eq!(
            result.unwrap_err().reason(),
            AliasUncertainty::UntrustedMount,
            "canonical retargeting and complete mount identity changes remain uncertain"
        );
        assert_eq!(inventories, 2);
    }
    Ok(())
}

#[test]
fn ordinary_ambiguous_nested_or_nonroot_targets_never_bind() -> TestResult {
    let fixture = Fixture::new()?;
    let mut duplicate = fixture.table.clone();
    duplicate.extend(mount_line(99, &fixture.first));
    let child = fixture.first.join("child");
    fs::create_dir(&child)?;
    for (raw, table) in [
        (
            fixture.raw(),
            b"21 1 8:1 / / rw - ext4 /dev/root rw\n".to_vec(),
        ),
        (fixture.raw(), duplicate),
        (bytestring(&child), fixture.table.clone()),
    ] {
        assert_eq!(
            MountBinding::capture(&raw, &AtomicBool::new(false), || Ok(table.clone()))
                .unwrap_err()
                .reason(),
            AliasUncertainty::UntrustedMount
        );
    }
    let mut nested = fixture.table;
    let mut line = b"88 42 0:88 / ".to_vec();
    line.extend_from_slice(child.as_os_str().as_bytes());
    line.extend_from_slice(b" rw - tmpfs tmpfs rw\n");
    nested.extend(line);
    assert_eq!(
        MountBinding::capture(&bytestring(&child), &AtomicBool::new(false), || Ok(
            nested.clone()
        ))
        .unwrap_err()
        .reason(),
        AliasUncertainty::UntrustedMount
    );
    Ok(())
}

#[test]
fn absent_broken_or_nondirectory_mountpoints_remain_unknown() -> TestResult {
    let fixture = Fixture::new()?;
    let file = fixture.root.path().join("file");
    fs::write(&file, b"synthetic")?;
    assert_eq!(
        MountBinding::capture(&bytestring(&file), &AtomicBool::new(false), || Ok(fixture
            .table
            .clone()))
        .unwrap_err()
        .reason(),
        AliasUncertainty::UntrustedMount
    );
    fs::remove_file(&fixture.declared)?;
    symlink(fixture.root.path().join("missing"), &fixture.declared)?;
    assert_eq!(
        MountBinding::capture(&fixture.raw(), &AtomicBool::new(false), || Ok(fixture
            .table
            .clone()))
        .unwrap_err()
        .reason(),
        AliasUncertainty::Filesystem
    );
    Ok(())
}

#[test]
fn the_actual_cancellation_flag_is_observed_before_and_between_binding_steps() -> TestResult {
    let fixture = Fixture::new()?;
    for stop_at in 0..=2 {
        let cancel = AtomicBool::new(stop_at == 0);
        let mut inventories = 0;
        let failure = MountBinding::capture(&fixture.raw(), &cancel, || {
            inventories += 1;
            if inventories == stop_at {
                cancel.store(true, Ordering::Release);
            }
            Ok(fixture.table.clone())
        })
        .unwrap_err();
        assert_eq!(failure.reason(), AliasUncertainty::Cancelled);
        assert_eq!(
            inventories, stop_at,
            "a cancelled operation does not proceed to another inventory or filesystem step"
        );
    }
    Ok(())
}

#[test]
fn inventory_io_failures_keep_their_original_cause() -> TestResult {
    let fixture = Fixture::new()?;
    let failure = MountBinding::capture(&fixture.raw(), &AtomicBool::new(false), || {
        Err(UnknownAlias::with_source(
            AliasUncertainty::MountInventory,
            io::Error::from(io::ErrorKind::PermissionDenied),
        ))
    })
    .unwrap_err();
    assert_eq!(failure.reason(), AliasUncertainty::MountInventory);
    assert_eq!(
        failure
            .source()
            .and_then(|source| source.downcast_ref::<io::Error>())
            .map(io::Error::kind),
        Some(io::ErrorKind::PermissionDenied)
    );
    Ok(())
}
