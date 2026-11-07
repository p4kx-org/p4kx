// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bound_documents_reply::BoundDocumentsReply;
use crate::portal_alias::{AliasUncertainty, DocumentGrant, UnknownAlias};
use crate::{portal_mount, portal_path};

#[cfg(test)]
mod tests;

pub(crate) const MAX_REPLY_BYTES: usize = 16 * 1024;
pub(crate) const DOCUMENTS_INTERFACE_HOST_PATHS: u32 = 5;

#[derive(Debug)]
pub(crate) struct DocumentsReply {
    pub(crate) owner_before: String,
    pub(crate) owner_after: String,
    pub(crate) interface: u32,
    pub(crate) mount_point: Vec<u8>,
    pub(crate) body_bytes: usize,
    pub(crate) paths: Vec<(String, Vec<u8>)>,
}

pub(crate) trait DocumentsTransport {
    fn lookup(&mut self, document_id: &str) -> Result<BoundDocumentsReply, UnknownAlias>;
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PortalMapping {
    pub(crate) host_path: Vec<u8>,
    pub(crate) document_id: String,
    pub(crate) grant: DocumentGrant,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MappingOutcome {
    NotPortal,
    Resolved(PortalMapping),
}

pub(crate) fn resolve_with(
    path: &[u8],
    mounts: &[u8],
    expected_mount: &[u8],
    grant: DocumentGrant,
    transport: &mut impl DocumentsTransport,
) -> Result<MappingOutcome, UnknownAlias> {
    let mount = portal_mount::covering_mount(mounts, path)?
        .ok_or_else(|| UnknownAlias::new(AliasUncertainty::MountInventory))?;
    if !mount.is_documents() {
        if mount.filesystem == b"fuse.portal"
            || path == expected_mount
            || portal_path::strip_mount(path, expected_mount).is_some()
        {
            return Err(UnknownAlias::new(AliasUncertainty::UntrustedMount));
        }
        return Ok(MappingOutcome::NotPortal);
    }
    let document = portal_path::document_path(path, &mount.target)?;
    let bound = transport.lookup(&document.id)?;
    bound.check_mount(&mount)?;
    let reply = bound.reply();
    let host = match reply.paths.as_slice() {
        [] => return Err(UnknownAlias::new(AliasUncertainty::MissingDocument)),
        [(id, path)] if id == &document.id => path,
        _ => return Err(UnknownAlias::new(AliasUncertainty::MalformedReply)),
    };
    let host_path = portal_path::mapped_descendant(&document, host, grant)?;
    Ok(MappingOutcome::Resolved(PortalMapping {
        host_path,
        document_id: document.id,
        grant,
    }))
}

impl DocumentsReply {
    pub(crate) fn validate(&self) -> Result<(), UnknownAlias> {
        if self.body_bytes > MAX_REPLY_BYTES {
            return Err(UnknownAlias::new(AliasUncertainty::Limit));
        }
        if self.owner_before.is_empty() || self.owner_before != self.owner_after {
            return Err(UnknownAlias::new(AliasUncertainty::OwnerChanged));
        }
        if self.interface < DOCUMENTS_INTERFACE_HOST_PATHS {
            return Err(UnknownAlias::new(AliasUncertainty::UnsupportedInterface(
                self.interface,
            )));
        }
        portal_path::bytestring_path(&self.mount_point)?;
        Ok(())
    }
}
