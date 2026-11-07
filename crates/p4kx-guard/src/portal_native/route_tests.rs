// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

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

    fn resolve(
        &self,
        mutation: impl FnMut() -> io::Result<()>,
    ) -> (Result<Option<HostAlias>, UnknownAlias>, usize, usize) {
        let mut transport = Lookup {
            mutation,
            mount: &self.mount,
            calls: 0,
        };
        let mut inventories = 0;
        let result = super::resolve_observed(
            &self.root.path().join("outer-link/new/output"),
            &AtomicBool::new(false),
            &self.mount,
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
    mount: &'mount std::path::Path,
    calls: usize,
}

impl<Mutation: FnMut() -> io::Result<()>> DocumentsTransport for Lookup<'_, Mutation> {
    fn lookup(&mut self, document_id: &str) -> Result<DocumentsReply, UnknownAlias> {
        self.calls += 1;
        assert_eq!(
            document_id, "aabbccdd",
            "the initial observed route selects grant A"
        );
        (self.mutation)()
            .map_err(|error| UnknownAlias::with_source(AliasUncertainty::Filesystem, error))?;
        let mut mount_point = self.mount.as_os_str().as_bytes().to_vec();
        mount_point.push(0);
        Ok(DocumentsReply {
            owner_before: ":1.7".to_owned(),
            owner_after: ":1.7".to_owned(),
            interface: 5,
            mount_point,
            body_bytes: 128,
            paths: vec![(
                document_id.to_owned(),
                b"/unobservable/host/exports\0".to_vec(),
            )],
        })
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
        std::path::Path::new("/unobservable/host/exports/new/output"),
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
