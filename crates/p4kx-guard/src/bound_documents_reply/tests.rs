// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::BoundDocumentsReply;
use crate::portal_alias::{AliasUncertainty, DocumentGrant, UnknownAlias};
use crate::portal_mount_binding::MountBinding;
use crate::portal_resolution::{self, DocumentsReply, DocumentsTransport, MappingOutcome};

const DECLARED: &[u8] = b"/run/user/1001/doc";
const VISIBLE: &[u8] = b"/run/flatpak/doc";
const MOUNTS: &[u8] = b"21 1 8:1 / / rw - ext4 /dev/root rw\n42 21 0:7 /by-app/example /run/flatpak/doc rw - fuse.portal portal rw\n";

fn reply() -> DocumentsReply {
    DocumentsReply {
        owner_before: ":1.7".into(),
        owner_after: ":1.7".into(),
        interface: 5,
        mount_point: b"/run/user/1001/doc\0".to_vec(),
        body_bytes: 128,
        paths: Vec::new(),
    }
}

fn bind(reply: DocumentsReply) -> Result<BoundDocumentsReply, UnknownAlias> {
    BoundDocumentsReply::synthetic(reply, MountBinding::synthetic(DECLARED, MOUNTS, VISIBLE)?)
}

struct Transport(Option<BoundDocumentsReply>);

impl DocumentsTransport for Transport {
    fn lookup(&mut self, id: &str) -> Result<BoundDocumentsReply, UnknownAlias> {
        assert_eq!(id, "aabbccdd", "the selected document is queried");
        self.0
            .take()
            .ok_or_else(|| UnknownAlias::new(AliasUncertainty::MissingDocument))
    }
}

#[test]
fn a_bound_probe_and_mapping_preserve_raw_bytes_and_accept_only_the_visible_kernel_identity() {
    let mut raw = reply();
    let bound = bind(raw).unwrap();
    bound.probe().unwrap();
    assert_eq!(
        bound.reply().mount_point,
        b"/run/user/1001/doc\0",
        "the codec's authenticated bytes are unchanged"
    );
    raw = reply();
    raw.paths = vec![("aabbccdd".into(), b"/host/exports\0".to_vec())];
    let mut transport = Transport(Some(bind(raw).unwrap()));
    let result = portal_resolution::resolve_with(
        b"/run/flatpak/doc/aabbccdd/exports/new",
        MOUNTS,
        DECLARED,
        DocumentGrant::Directory,
        &mut transport,
    )
    .unwrap();
    assert!(
        matches!(result, MappingOutcome::Resolved(mapping) if mapping.host_path == b"/host/exports/new")
    );
}

#[test]
fn a_stale_bound_mount_cannot_project_a_host_path_even_with_the_same_target_spelling() {
    let mut raw = reply();
    raw.paths = vec![("aabbccdd".into(), b"/host/exports\0".to_vec())];
    let mut transport = Transport(Some(bind(raw).unwrap()));
    let changed = b"21 1 8:1 / / rw - ext4 /dev/root rw\n43 21 0:8 /by-app/example /run/flatpak/doc rw - fuse.portal portal rw\n";
    assert_eq!(
        portal_resolution::resolve_with(
            b"/run/flatpak/doc/aabbccdd/exports/new",
            changed,
            DECLARED,
            DocumentGrant::Directory,
            &mut transport
        )
        .unwrap_err()
        .reason(),
        AliasUncertainty::UntrustedMount
    );
}

#[test]
fn changed_owner_old_api_oversized_and_differently_declared_replies_remain_refusals() {
    type Edit = fn(&mut DocumentsReply);
    let cases: &[(Edit, AliasUncertainty)] = &[
        (
            |reply| reply.owner_after = ":1.8".into(),
            AliasUncertainty::OwnerChanged,
        ),
        (
            |reply| reply.interface = 4,
            AliasUncertainty::UnsupportedInterface(4),
        ),
        (
            |reply| reply.body_bytes = portal_resolution::MAX_REPLY_BYTES + 1,
            AliasUncertainty::Limit,
        ),
        (
            |reply| reply.mount_point = b"/different\0".to_vec(),
            AliasUncertainty::UntrustedMount,
        ),
        (
            |reply| reply.mount_point = b"relative\0".to_vec(),
            AliasUncertainty::MalformedReply,
        ),
    ];
    for (edit, expected) in cases {
        let mut raw = reply();
        edit(&mut raw);
        assert_eq!(bind(raw).unwrap_err().reason(), *expected);
    }
}

#[test]
fn a_nonempty_capability_reply_cannot_be_treated_as_an_empty_probe() {
    let mut raw = reply();
    raw.paths = vec![("aabbccdd".into(), b"/host/exports\0".to_vec())];
    assert_eq!(
        bind(raw).unwrap().probe().unwrap_err().reason(),
        AliasUncertainty::MalformedReply
    );
}
