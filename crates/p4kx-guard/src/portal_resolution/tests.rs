// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{DocumentsReply, DocumentsTransport, MappingOutcome, PortalMapping};
use crate::portal_alias::{AliasUncertainty, DocumentGrant, HostAlias, PortalAlias, UnknownAlias};

const MOUNT: &[u8] = b"/run/user/1001/doc";
const MOUNTS: &[u8] = b"21 1 8:1 / / rw - ext4 /dev/root rw\n42 21 0:7 / /run/user/1001/doc rw - fuse.portal portal rw\n";
const TARGET: &[u8] = b"/run/user/1001/doc/aabbccdd/exports/new/file.txt";
type ReplyMutation = fn(&mut DocumentsReply);

struct Transport {
    result: Option<Result<DocumentsReply, UnknownAlias>>,
    requested: Vec<String>,
}

impl Transport {
    fn reply() -> Self {
        Self {
            result: Some(Ok(DocumentsReply {
                owner_before: ":1.7".to_owned(),
                owner_after: ":1.7".to_owned(),
                interface: 5,
                mount_point: b"/run/user/1001/doc\0".to_vec(),
                body_bytes: 128,
                paths: vec![("aabbccdd".to_owned(), b"/home/user/exports\0".to_vec())],
            })),
            requested: Vec::new(),
        }
    }

    fn edit(&mut self, edit: impl FnOnce(&mut DocumentsReply)) {
        edit(self.result.as_mut().unwrap().as_mut().unwrap());
    }

    fn resolve(&mut self) -> Result<MappingOutcome, UnknownAlias> {
        super::resolve_with(TARGET, MOUNTS, MOUNT, DocumentGrant::Directory, self)
    }
}

impl DocumentsTransport for Transport {
    fn lookup(&mut self, document_id: &str) -> Result<DocumentsReply, UnknownAlias> {
        self.requested.push(document_id.to_owned());
        self.result.take().expect("one document query")
    }
}

#[test]
fn a_directory_mapping_keeps_only_its_validated_suffix() {
    let mut transport = Transport::reply();
    let result = transport.resolve().unwrap();
    assert_eq!(
        result,
        MappingOutcome::Resolved(PortalMapping {
            host_path: b"/home/user/exports/new/file.txt".to_vec(),
            document_id: "aabbccdd".to_owned(),
            grant: DocumentGrant::Directory,
        }),
        "descendants belong to the authenticated document root"
    );
    assert_eq!(
        transport.requested,
        ["aabbccdd"],
        "only the exact source document is queried"
    );
}

#[test]
fn a_source_file_grant_maps_no_sibling_or_installation() {
    let mut transport = Transport::reply();
    transport.edit(|reply| {
        reply.paths = vec![("aabbccdd".to_owned(), b"/custom/LIVE/Data.p4k\0".to_vec())];
    });
    let result = super::resolve_with(
        b"/run/user/1001/doc/aabbccdd/Data.p4k",
        MOUNTS,
        MOUNT,
        DocumentGrant::File,
        &mut transport,
    )
    .unwrap();
    assert_eq!(
        result,
        MappingOutcome::Resolved(PortalMapping {
            host_path: b"/custom/LIVE/Data.p4k".to_vec(),
            document_id: "aabbccdd".to_owned(),
            grant: DocumentGrant::File,
        }),
        "a file grant proves its path, without inventing a parent installation"
    );
}

#[test]
fn ordinary_paths_and_foreign_fuse_do_not_consult_document_hints() {
    for table in [
        b"21 1 8:1 / / rw - ext4 /dev/root rw\n".as_slice(),
        b"21 1 0:9 / / rw - fuse.sshfs unrelated rw\n",
    ] {
        let mut transport = Transport::reply();
        let result = super::resolve_with(
            b"/tmp/forged-host-path-xattr",
            table,
            MOUNT,
            DocumentGrant::File,
            &mut transport,
        )
        .unwrap();
        assert_eq!(
            result,
            MappingOutcome::NotPortal,
            "an xattr-like hint is not a document mapping"
        );
        assert_eq!(
            transport.requested,
            Vec::<String>::new(),
            "ordinary paths do not call the portal"
        );
    }
}

#[test]
fn fake_mount_spelling_and_parent_suffixes_are_uncertain() {
    let mut transport = Transport::reply();
    let reason = super::resolve_with(
        TARGET,
        b"21 1 8:1 / / rw - ext4 /dev/root rw\n",
        MOUNT,
        DocumentGrant::Directory,
        &mut transport,
    )
    .unwrap_err()
    .reason();
    assert_eq!(
        reason,
        AliasUncertainty::UntrustedMount,
        "the runtime-directory spelling alone is not authority"
    );
    for path in [
        b"/run/user/1001/doc/aabbccdd/exports/../LIVE".as_slice(),
        b"/run/user/1001/doc/../aabbccdd/exports",
        b"/run/user/1001/doc/not-hex/exports",
    ] {
        let mut transport = Transport::reply();
        assert_eq!(
            super::resolve_with(
                path,
                MOUNTS,
                MOUNT,
                DocumentGrant::Directory,
                &mut transport
            )
            .unwrap_err()
            .reason(),
            AliasUncertainty::DocumentPath,
            "a grant cannot escape through lexical components"
        );
        assert_eq!(
            transport.requested,
            Vec::<String>::new(),
            "invalid paths never trigger service calls"
        );
    }
}

#[test]
fn version_four_owner_changes_limits_and_ambiguous_paths_never_resolve() {
    let cases: &[(ReplyMutation, AliasUncertainty)] = &[
        (
            |reply| reply.interface = 4,
            AliasUncertainty::UnsupportedInterface(4),
        ),
        (
            |reply| reply.owner_after = ":1.8".to_owned(),
            AliasUncertainty::OwnerChanged,
        ),
        (
            |reply| reply.body_bytes = super::MAX_REPLY_BYTES + 1,
            AliasUncertainty::Limit,
        ),
        (
            |reply| reply.mount_point = b"/different\0".to_vec(),
            AliasUncertainty::UntrustedMount,
        ),
        (
            |reply| reply.paths.clear(),
            AliasUncertainty::MissingDocument,
        ),
        (
            |reply| reply.paths.push(reply.paths[0].clone()),
            AliasUncertainty::MalformedReply,
        ),
        (
            |reply| reply.paths[0].0 = "deadbeef".to_owned(),
            AliasUncertainty::MalformedReply,
        ),
        (
            |reply| reply.paths[0].1 = b"relative/exports\0".to_vec(),
            AliasUncertainty::MalformedReply,
        ),
        (
            |reply| reply.paths[0].1 = b"/home/user/exports\0extra\0".to_vec(),
            AliasUncertainty::MalformedReply,
        ),
    ];
    for (edit, expected) in cases {
        let mut transport = Transport::reply();
        transport.edit(edit);
        assert_eq!(
            transport.resolve().unwrap_err().reason(),
            *expected,
            "uncertainty cannot become an allowed alias"
        );
    }
}

#[test]
fn denial_timeout_and_malformed_transport_results_stay_typed() {
    for reason in [
        AliasUncertainty::AccessDenied,
        AliasUncertainty::Timeout,
        AliasUncertainty::MalformedReply,
        AliasUncertainty::UnsupportedMethod,
    ] {
        let mut transport = Transport {
            result: Some(Err(UnknownAlias::new(reason))),
            requested: Vec::new(),
        };
        assert_eq!(
            transport.resolve().unwrap_err().reason(),
            reason,
            "transport failure does not certify an outside path"
        );
    }
}

#[test]
fn resolved_public_identity_keeps_the_grant_kind() {
    let alias = HostAlias::new(
        std::path::PathBuf::from("/outside/file"),
        "abcd".to_owned(),
        DocumentGrant::File,
    );
    assert_eq!(
        alias.document_id(),
        "abcd",
        "document identity is retained for diagnostics"
    );
    assert_eq!(
        alias.grant(),
        DocumentGrant::File,
        "a source-file grant does not acquire sibling visibility"
    );
    let outcome = PortalAlias::Resolved(alias);
    assert!(
        matches!(outcome, PortalAlias::Resolved(_)),
        "the public result represents evidence, not a write permission"
    );
}

#[test]
fn an_empty_protection_set_does_not_change_source_identity_evidence() {
    let installation = crate::Installation::new();
    assert!(
        installation.is_empty(),
        "a file-only grant can hide the host installation"
    );
    let mut transport = Transport::reply();
    transport.edit(|reply| {
        reply.paths = vec![(
            "aabbccdd".to_owned(),
            b"/unobservable/LIVE/Data.p4k\0".to_vec(),
        )];
    });
    let mapped = super::resolve_with(
        b"/run/user/1001/doc/aabbccdd/Data.p4k",
        MOUNTS,
        MOUNT,
        DocumentGrant::File,
        &mut transport,
    )
    .unwrap();
    assert!(
        matches!(mapped, MappingOutcome::Resolved(mapping) if mapping.host_path == b"/unobservable/LIVE/Data.p4k"),
        "host spelling is established without inspecting a hidden host parent"
    );
    let mut unavailable = Transport::reply();
    unavailable.edit(|reply| reply.interface = 4);
    assert_eq!(
        unavailable.resolve().unwrap_err().reason(),
        AliasUncertainty::UnsupportedInterface(4),
        "empty protection discovery does not turn an unavailable identity into a trusted negative"
    );
    assert!(
        installation.is_empty(),
        "neither a Data.p4k name nor a resolved host spelling classifies an installation"
    );
}
