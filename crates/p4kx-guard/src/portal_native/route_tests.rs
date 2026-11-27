// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bound_documents_reply::BoundDocumentsReply;
use crate::portal_alias::{AliasUncertainty, HostAlias, UnknownAlias};
use crate::portal_filesystem::GrantSnapshot;
use crate::portal_path::DocumentPath;
use crate::portal_resolution::{DocumentsReply, DocumentsTransport};

type TestResult = Result<(), Box<dyn Error>>;

struct RouteFixture {
    root: tempfile::TempDir,
    mount: PathBuf,
    table: Vec<u8>,
    document: DocumentPath,
}

impl RouteFixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let root = tempfile::tempdir()?;
        let mount = root.path().join("doc");
        fs::create_dir_all(mount.join("aabbccdd/exports"))?;
        fs::create_dir_all(mount.join("deadbeef/exports"))?;
        fs::create_dir(root.path().join("ordinary"))?;
        symlink(
            mount.join("aabbccdd/exports"),
            root.path().join("outer-link"),
        )?;
        let mut table = b"21 1 8:1 / / rw - ext4 /dev/root rw\n42 21 0:7 / ".to_vec();
        table.extend_from_slice(mount.as_os_str().as_bytes());
        table.extend_from_slice(b" rw - fuse.portal portal rw\n");
        let document = DocumentPath {
            id: "aabbccdd".to_owned(),
            name: b"exports".to_vec(),
            suffix: vec![b"new".to_vec(), b"output".to_vec()],
        };
        Ok(Self {
            root,
            mount,
            table,
            document,
        })
    }

    fn snapshot(&self) -> Result<GrantSnapshot, UnknownAlias> {
        GrantSnapshot::capture(self.mount.as_os_str().as_bytes(), &self.document)
    }

    fn runtime_alias(&self) -> Result<PathBuf, io::Error> {
        let directory = self.root.path().join("runtime");
        fs::create_dir(&directory)?;
        let declared = directory.join("doc");
        symlink(&self.mount, &declared)?;
        Ok(declared)
    }

    fn resolve(
        &self,
        mutation: impl FnMut() -> io::Result<()>,
    ) -> (Result<Option<HostAlias>, UnknownAlias>, usize, usize) {
        self.resolve_path(
            &self.root.path().join("outer-link/new/output"),
            &self.mount,
            mutation,
        )
    }

    fn resolve_path(
        &self,
        path: &Path,
        declared: &Path,
        mutation: impl FnMut() -> io::Result<()>,
    ) -> (Result<Option<HostAlias>, UnknownAlias>, usize, usize) {
        self.resolve_with_cancel(path, declared, &AtomicBool::new(false), mutation)
    }

    fn resolve_with_cancel(
        &self,
        path: &Path,
        declared: &Path,
        cancel: &AtomicBool,
        mutation: impl FnMut() -> io::Result<()>,
    ) -> (Result<Option<HostAlias>, UnknownAlias>, usize, usize) {
        let mut transport = Lookup {
            mutation,
            declared,
            table: &self.table,
            cancel,
            calls: 0,
        };
        let mut inventories = 0;
        let result = super::resolve_observed(
            path,
            cancel,
            declared,
            || {
                inventories += 1;
                Ok(self.table.clone())
            },
            &mut transport,
        );
        (result, transport.calls, inventories)
    }
}

struct Lookup<'mount, Mutation> {
    mutation: Mutation,
    declared: &'mount Path,
    table: &'mount [u8],
    cancel: &'mount AtomicBool,
    calls: usize,
}

impl<Mutation: FnMut() -> io::Result<()>> DocumentsTransport for Lookup<'_, Mutation> {
    fn lookup(&mut self, document_id: &str) -> Result<BoundDocumentsReply, UnknownAlias> {
        self.calls += 1;
        assert_eq!(
            document_id, "aabbccdd",
            "the initial observed route selects grant A"
        );
        (self.mutation)()
            .map_err(|error| UnknownAlias::with_source(AliasUncertainty::Filesystem, error))?;
        let mut mount_point = self.declared.as_os_str().as_bytes().to_vec();
        mount_point.push(0);
        let reply = DocumentsReply {
            owner_before: ":1.7".to_owned(),
            owner_after: ":1.7".to_owned(),
            interface: 5,
            mount_point,
            body_bytes: 128,
            paths: vec![(
                document_id.to_owned(),
                b"/unobservable/host/exports\0".to_vec(),
            )],
        };
        BoundDocumentsReply::bind(reply, self.cancel, || Ok(self.table.to_vec()))
    }
}

#[test]
fn changing_the_original_route_during_lookup_is_unknown_even_when_grant_a_is_stable() -> TestResult
{
    for destination in ["doc/deadbeef/exports", "ordinary"] {
        let fixture = RouteFixture::new()?;
        let before = fixture.snapshot()?;
        let link = fixture.root.path().join("outer-link");
        let changed = fixture.root.path().join(destination);
        let (result, calls, inventories) = fixture.resolve(|| {
            fs::remove_file(&link)?;
            symlink(&changed, &link)
        });
        assert_eq!(
            result.unwrap_err().reason(),
            AliasUncertainty::UntrustedMount,
            "an authenticated mapping of grant A cannot describe the input's changed route"
        );
        assert_eq!(
            calls, 1,
            "retargeting happens inside the actual query boundary"
        );
        assert_eq!(
            inventories, 2,
            "the original absolute input is reclassified against a fresh inventory"
        );
        assert_eq!(
            fixture.snapshot()?,
            before,
            "grant A's device/inode observations alone would miss the outer-link change"
        );
    }
    Ok(())
}

#[test]
fn an_unchanged_original_alias_can_resolve_an_absent_output_suffix() -> TestResult {
    let fixture = RouteFixture::new()?;
    let (result, calls, inventories) = fixture.resolve(|| Ok(()));
    let alias = result?.ok_or("an unchanged observed portal route was lost")?;
    assert_eq!(
        alias.path(),
        Path::new("/unobservable/host/exports/new/output"),
        "the complete query maps the unchanged input route without inspecting the hidden host parent"
    );
    assert_eq!(calls, 1, "one authenticated origin query is sufficient");
    assert_eq!(
        inventories, 2,
        "the positive case also revalidates its original route"
    );
    assert!(
        !fixture.mount.join("aabbccdd/exports/new").exists(),
        "resolution creates no destination"
    );
    Ok(())
}

#[test]
fn the_runtime_alias_reaches_the_shared_binding_and_complete_mapping_route() -> TestResult {
    let fixture = RouteFixture::new()?;
    let declared = fixture.runtime_alias()?;
    let input = declared.join("aabbccdd/exports/new/output");
    let (result, calls, inventories) = fixture.resolve_path(&input, &declared, || Ok(()));
    let alias = result?.ok_or("the visible runtime alias was lost")?;
    assert_eq!(
        alias.path(),
        Path::new("/unobservable/host/exports/new/output")
    );
    assert_eq!(
        calls, 1,
        "the actual mapping route invokes one document lookup and its shared mount binder"
    );
    assert_eq!(
        inventories, 2,
        "the original runtime spelling is reclassified after lookup"
    );
    assert!(!fixture.mount.join("aabbccdd/exports/new").exists());
    Ok(())
}

#[test]
fn retargeting_the_runtime_alias_never_publishes_the_stable_grant_a_mapping() -> TestResult {
    let fixture = RouteFixture::new()?;
    let declared = fixture.runtime_alias()?;
    let before = fixture.snapshot()?;
    let (result, calls, _) = fixture.resolve_path(
        &declared.join("aabbccdd/exports/new/output"),
        &declared,
        || {
            fs::remove_file(&declared)?;
            symlink(fixture.root.path().join("ordinary"), &declared)
        },
    );
    assert_eq!(
        result.unwrap_err().reason(),
        AliasUncertainty::UntrustedMount
    );
    assert_eq!(calls, 1);
    assert_eq!(
        fixture.snapshot()?,
        before,
        "stable proxy identities alone cannot authenticate a changed runtime route"
    );
    Ok(())
}

#[test]
fn canonicalization_does_not_hide_forbidden_portal_suffixes() -> TestResult {
    let fixture = RouteFixture::new()?;
    let declared = fixture.runtime_alias()?;
    for suffix in [
        "aabbccdd/exports/../exports/new",
        "aabbccdd/exports/./new",
        "aabbccdd/exports//new",
    ] {
        let (result, calls, _) = fixture.resolve_path(&declared.join(suffix), &declared, || Ok(()));
        assert_eq!(result.unwrap_err().reason(), AliasUncertainty::DocumentPath);
        assert_eq!(
            calls, 0,
            "invalid suffix spelling is refused before consulting any host-path mapping"
        );
    }
    Ok(())
}

#[test]
fn a_foreign_nested_mount_below_the_runtime_alias_remains_unknown() -> TestResult {
    let mut fixture = RouteFixture::new()?;
    let declared = fixture.runtime_alias()?;
    let target = fixture.mount.join("aabbccdd/exports");
    fixture.table.extend_from_slice(b"99 42 0:99 / ");
    fixture
        .table
        .extend_from_slice(target.as_os_str().as_bytes());
    fixture.table.extend_from_slice(b" rw - tmpfs tmpfs rw\n");
    let (result, calls, _) = fixture.resolve_path(
        &declared.join("aabbccdd/exports/new/output"),
        &declared,
        || Ok(()),
    );
    assert_eq!(
        result.unwrap_err().reason(),
        AliasUncertainty::UntrustedMount
    );
    assert_eq!(calls, 0);
    Ok(())
}

#[test]
fn ordinary_outside_paths_do_not_depend_on_a_broken_expected_runtime_alias() -> TestResult {
    let fixture = RouteFixture::new()?;
    let declared = fixture.root.path().join("broken-runtime");
    symlink(&declared, &declared)?;
    let (result, calls, inventories) =
        fixture.resolve_path(&fixture.root.path().join("ordinary/new"), &declared, || {
            Ok(())
        });
    assert!(
        result?.is_none(),
        "an unrelated expected-doc alias cannot globally block ordinary outside paths"
    );
    assert_eq!(calls, 0);
    assert_eq!(inventories, 1);
    Ok(())
}

#[test]
fn cancellation_during_lookup_is_shared_with_binding_and_prevents_publication() -> TestResult {
    let fixture = RouteFixture::new()?;
    let declared = fixture.runtime_alias()?;
    let cancel = AtomicBool::new(false);
    let (result, calls, _) = fixture.resolve_with_cancel(
        &declared.join("aabbccdd/exports/new/output"),
        &declared,
        &cancel,
        || {
            cancel.store(true, Ordering::Release);
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err().reason(), AliasUncertainty::Cancelled);
    assert_eq!(calls, 1);
    assert!(cancel.load(Ordering::Acquire));
    Ok(())
}

mod nofollow;
