// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::portal_resolution::{DocumentsReply, DocumentsTransport, MappingOutcome};
use crate::{
    AliasUncertainty, ArchiveWriteProtection, DocumentGrant, HostAlias, PortalAlias, UnknownAlias,
    WriteProtectionError, portal_resolution,
};

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;

const MOUNT: &[u8] = b"/run/user/1/doc";
const MOUNTS: &[u8] =
    b"1 0 0:1 / / rw - ext4 /dev/root rw\n2 1 0:2 / /run/user/1/doc rw - fuse.portal portal rw\n";
const HOST: &[u8] = b"/host/Exports";

#[derive(Clone, Copy)]
pub(super) enum SourceKind {
    Ordinary,
    FileGrant,
}

pub(super) const SOURCES: [SourceKind; 2] = [SourceKind::Ordinary, SourceKind::FileGrant];

pub(super) struct Fixture {
    pub(super) folder: tempfile::TempDir,
    pub(super) visible: PathBuf,
    pub(super) host: PathBuf,
    pub(super) source: PathBuf,
    pub(super) proxy: PathBuf,
    pub(super) host_source: PathBuf,
}

impl Fixture {
    pub(super) fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let folder = tempfile::tempdir()?;
        let visible = folder.path().join("view").join("abcd").join("Exports");
        let host = folder.path().join("host").join("Exports");
        let source = folder.path().join("ordinary-source.p4k");
        let proxy = folder.path().join("source-proxy.p4k");
        let host_source = folder.path().join("hidden-source").join("Data.p4k");
        for path in [
            visible.as_path(),
            host.as_path(),
            host_source.parent().ok_or("source parent")?,
        ] {
            fs::create_dir_all(path)?;
        }
        for path in [&source, &proxy, &host_source] {
            fs::write(path, b"synthetic source bytes")?;
        }
        Ok(Self {
            folder,
            visible,
            host: fs::canonicalize(host)?,
            source: fs::canonicalize(source)?,
            proxy: fs::canonicalize(proxy)?,
            host_source: fs::canonicalize(host_source)?,
        })
    }

    pub(super) fn protection(
        &self,
        kind: SourceKind,
    ) -> Result<ArchiveWriteProtection, WriteProtectionError> {
        match kind {
            SourceKind::Ordinary => {
                ArchiveWriteProtection::new(&self.source, &AtomicBool::new(false))
            }
            SourceKind::FileGrant => ArchiveWriteProtection::from_alias(
                &self.proxy,
                PortalAlias::Resolved(HostAlias::new(
                    self.host_source.clone(),
                    "1234".into(),
                    DocumentGrant::File,
                )),
            ),
        }
    }

    pub(super) fn source_for(&self, kind: SourceKind) -> &Path {
        match kind {
            SourceKind::Ordinary => &self.source,
            SourceKind::FileGrant => &self.proxy,
        }
    }

    pub(super) fn lookup(
        &self,
        path: &Path,
        cancel: &AtomicBool,
    ) -> Result<PathBuf, WriteProtectionError> {
        if cancel.load(Ordering::Acquire) {
            return Err(refused(path, AliasUncertainty::Cancelled));
        }
        let relative = path
            .strip_prefix(&self.visible)
            .map_err(|_| refused(path, AliasUncertainty::DocumentPath))?;
        let mut portal = b"/run/user/1/doc/abcd/Exports".to_vec();
        for component in relative.components() {
            let name = component
                .as_os_str()
                .to_str()
                .ok_or_else(|| refused(path, AliasUncertainty::DocumentPath))?;
            portal.push(b'/');
            portal.extend_from_slice(name.as_bytes());
        }
        let mapped = portal_resolution::resolve_with(
            &portal,
            MOUNTS,
            MOUNT,
            DocumentGrant::Directory,
            &mut FixedTransport,
        )
        .map_err(|cause| WriteProtectionError::TargetIdentity {
            target: path.into(),
            cause,
        })?;
        let MappingOutcome::Resolved(mapped) = mapped else {
            return Err(refused(path, AliasUncertainty::UntrustedMount));
        };
        let suffix = mapped
            .host_path
            .strip_prefix(HOST)
            .ok_or_else(|| refused(path, AliasUncertainty::MalformedReply))?;
        let mut host = self.host.clone();
        for component in suffix
            .split(|byte| *byte == b'/')
            .filter(|component| !component.is_empty())
        {
            host.push(
                std::str::from_utf8(component)
                    .map_err(|_| refused(path, AliasUncertainty::MalformedReply))?,
            );
        }
        Ok(host)
    }
}

struct FixedTransport;

impl DocumentsTransport for FixedTransport {
    fn lookup(&mut self, document_id: &str) -> Result<DocumentsReply, UnknownAlias> {
        let mut host = HOST.to_vec();
        host.push(0);
        let mut mount = MOUNT.to_vec();
        mount.push(0);
        Ok(DocumentsReply {
            owner_before: ":1.20".into(),
            owner_after: ":1.20".into(),
            interface: 5,
            mount_point: mount,
            body_bytes: 128,
            paths: vec![(document_id.into(), host)],
        })
    }
}

pub(super) fn refused(path: &Path, reason: AliasUncertainty) -> WriteProtectionError {
    WriteProtectionError::TargetIdentity {
        target: path.into(),
        cause: UnknownAlias::new(reason),
    }
}
