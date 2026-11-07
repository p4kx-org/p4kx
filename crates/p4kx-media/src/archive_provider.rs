// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::EntryMetadataInfo;
use serde::Serialize;

use crate::{
    Availability, Capability, FieldGroup, Identity, MediaError, MediaKind, MediaWriter, Projection,
    Provider, RootSource, Source, SourceContext, Status, Subresource,
};

pub(crate) const ENTRY_NAMESPACE: &str = "archive-entry";

/// Permanent minimal provider of existing public Archive metadata.
pub struct ArchiveProvider;

impl Provider for ArchiveProvider {
    fn capability(&self) -> Capability {
        Capability { kind: MediaKind::Archive, id: "p4kx.archive.public-metadata".into(), version: "1.2.0".into(),
            availability: Availability::Available,
            preservation: "exact decoded selected Entry payloads; root Archive layout bytes are not retained".into(),
            field_groups: vec![
                FieldGroup { name: "public-entry-metadata".into(), projected: true, scope: "physical index, display path, available raw path bytes, sizes, declared checksum, compression, encryption, installed bytes, original DOS words, timezone-free calendar status, source record format, original directory ordinal and fixed header words".into() },
                FieldGroup { name: "root-build".into(), projected: true, scope: "adjacent public launcher Build declaration, without inferred values".into() },
                FieldGroup { name: "raw-archive-layout".into(), projected: false, scope: "directory layout, signatures and unknown root bytes require a separate raw-layout reader".into() },
            ] }
    }

    fn root(&self, root: &RootSource, writer: &mut MediaWriter<'_>) -> Result<(), MediaError> {
        let issues: Vec<_> = root
            .directory_issues
            .iter()
            .filter(|issue| issue.archive_wide)
            .map(|issue| crate::MediaIssue {
                code: issue.code.clone(),
                location: "archive.directory".into(),
                severity: crate::Severity::Warning,
                detail: issue.detail.clone(),
            })
            .collect();
        writer.record(
            &Subresource::new("archive-root", Vec::new()),
            if root.enumeration_complete {
                Status::Complete
            } else {
                Status::Partial
            },
            root,
            &issues,
        )
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        #[derive(Serialize)]
        struct EntryData {
            metadata: EntryMetadataInfo,
            compressed_size: u64,
            uncompressed_size: u64,
            declared_compression: String,
            encrypted: bool,
            installed_bytes: Option<u64>,
            stored_sha256: Option<String>,
            stored_sha256_checked_by_reader: bool,
        }
        let entry = context.entry;
        let data = EntryData {
            metadata: EntryMetadataInfo::of(entry),
            compressed_size: entry.compressed_size(),
            uncompressed_size: entry.uncompressed_size(),
            declared_compression: entry.compression().to_string(),
            encrypted: entry.is_encrypted(),
            installed_bytes: entry.installed_bytes(),
            stored_sha256: entry
                .sha256()
                .map(|digest| p4kx_extraction::Checksum::Sha256(digest).hex()),
            stored_sha256_checked_by_reader: entry.is_sha256_verified(),
        };
        writer.record(
            &Subresource::new(ENTRY_NAMESPACE, Vec::new()),
            Status::Complete,
            &data,
            &[],
        )?;
        Ok(Projection::Written)
    }
}

pub(crate) fn root_source(root: &RootSource) -> Source {
    Source {
        root: root.digest.clone(),
        identity: Identity::Root,
        size: root.size,
        declared_integrity: None,
        verification: crate::Verification::NotAvailable,
        raw: None,
    }
}
