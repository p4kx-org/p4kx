// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    FieldOrigin, InputScope, ObservedEntry, SourceEvidenceError, SourceInput, SourceInventory,
    SourceLocation, SourceUseRole, SourceUses, directory_path_uses,
};

use SourceUseRole::{Conversion, Payload};

/// Original sources of the bytes and metadata displayed in the Text and Hex tabs.
#[derive(Debug)]
#[non_exhaustive]
pub struct EntryPreviewProvenance {
    /// The selected Entry path in its owning directory.
    pub path: FieldOrigin,
    /// The declared uncompressed size of the Entry.
    pub size: FieldOrigin,
    /// The successfully retained bytes of the Entry.
    pub bytes: FieldOrigin,
    /// The bytes interpreted as the displayed Text view.
    pub text: FieldOrigin,
    /// The bytes formatted as the displayed Hex view.
    pub hex: FieldOrigin,
    /// The length of the retained prefix.
    pub shown_length: FieldOrigin,
    /// The comparison of the retained length and declared size.
    pub truncated: FieldOrigin,
    inventory: SourceInventory,
}

impl EntryPreviewProvenance {
    pub(super) fn capture(
        original: Option<ObservedEntry<'_>>,
        length: u64,
    ) -> Result<Self, SourceEvidenceError> {
        let uses = SourceUses::with_compact_use_refs();
        let complete = original.is_some_and(|entry| entry.ancestors().all(|step| step.is_some()));
        let ancestors = original.map_or(0, |entry| entry.ancestors().len());
        let input = original.map_or(SourceInput::Unknown, SourceInput::ArchiveView);
        let payload = uses.observe(input, InputScope::Payload)?;
        let directory = payload.for_scope(InputScope::ArchiveDirectoryMetadata);
        let (path_uses, path_complete) = directory_path_uses(&directory, ancestors, complete)?;
        let size_location = SourceLocation::Metadata {
            field: "uncompressed_size".into(),
        };
        let size = directory.at(size_location.clone(), Payload)?;
        let window = SourceLocation::archive_entry_decoded_bytes(0..length, "bytes");
        let bytes = payload.at(window.clone(), Payload)?;
        let conversion = payload.at(window, Conversion)?;
        let display = uses.origin(std::slice::from_ref(&conversion), complete);
        let size_conversion = directory.at(size_location, Conversion)?;
        Ok(Self {
            path: uses.origin(&path_uses, path_complete),
            size: uses.origin(&[size], complete),
            bytes: uses.origin(&[bytes], complete),
            text: display.clone(),
            hex: display.clone(),
            shown_length: display,
            truncated: uses.origin(&[conversion, size_conversion], complete),
            inventory: uses.freeze()?,
        })
    }

    /// Returns the physical Entry and exact contributor locations used by the preview.
    #[must_use]
    pub const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }
}

#[test]
fn unbound_bytes_keep_unknown_encryption_and_incomplete_contributors()
-> Result<(), SourceEvidenceError> {
    let proof = EntryPreviewProvenance::capture(None, 7)?;
    for origin in [
        &proof.path,
        &proof.size,
        &proof.bytes,
        &proof.text,
        &proof.hex,
        &proof.shown_length,
        &proof.truncated,
    ] {
        assert_eq!(
            origin.contains_encrypted_source(),
            None,
            "no original authority"
        );
        assert!(
            !origin.source_set_complete(),
            "no exhaustive physical proof"
        );
    }
    Ok(())
}
