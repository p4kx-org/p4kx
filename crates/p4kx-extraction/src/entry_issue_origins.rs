// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{
    Archive, Entry, EntryIssue, ObservedRecoveryDataOutsideFile, ObservedRecoveryZip64Selection,
    StoredEncryptionDeclaration,
};

use crate::{
    ArchiveLayoutOriginError, FieldOrigin, FieldOrigins, FieldPath, SourceDetails,
    SourceEvidenceError, SourceUseRole, SourceUses,
};

use super::directory_range;
use super::source::EntryDirectorySource;

pub(crate) fn write(
    archive: &Archive,
    issue_limit: usize,
    uses: &SourceUses,
    fields: &mut FieldOrigins,
    path: &mut FieldPath,
) -> Result<(), ArchiveLayoutOriginError> {
    if issue_limit == 0 || archive.root_directory_entry_count().is_none() {
        return Ok(());
    }
    path.push_field("issues")?;
    for (position, (index, issue)) in archive.issues().iter().take(issue_limit).enumerate() {
        path.push_index(position)?;
        let original = archive
            .entries()
            .get(*index)
            .filter(|_| !issue.is_archive_wide());
        let (path_origin, check_origin) = if let Some(entry) = original {
            observe(archive, entry, issue, position, uses)?
        } else {
            (uses.origin(&[], false), uses.origin(&[], false))
        };
        path.push_field("path")?;
        fields.insert(path.clone(), path_origin)?;
        path.pop();
        path.push_field("issue")?;
        fields.insert(path.clone(), check_origin)?;
        path.pop();
        path.pop();
    }
    path.pop();
    Ok(())
}

fn observe(
    archive: &Archive,
    entry: &Entry,
    issue: &EntryIssue,
    position: usize,
    uses: &SourceUses,
) -> Result<(FieldOrigin, FieldOrigin), SourceEvidenceError> {
    let source = EntryDirectorySource::observe(archive, entry, uses.clone(), SourceDetails::Full)?;
    let lossy = archive.raw_path(entry.index()).is_some();
    let path_origin = if lossy {
        let mut contributors = source.path_uses.clone();
        contributors.push(source.directory_fact("path", SourceUseRole::Conversion)?);
        source.origin(&contributors)
    } else {
        source.path.clone()
    };
    let check_origin = match *issue {
        EntryIssue::DataOutsideFile => {
            match archive.observed_recovery_data_outside_file(position) {
                Some(bounds) => recovery_data_range_origin(bounds, &source)?,
                None => v2_data_range_origin(archive, position, &source, uses)?,
            }
        }
        EntryIssue::HashBlockSize { .. } => hash_block_origin(archive, position, &source)?,
        EntryIssue::PathNotUtf8 if lossy => {
            let value = source.directory_fact("path", SourceUseRole::Conversion)?;
            source.origin(&[value])
        }
        EntryIssue::UnknownEncryption { value } => descriptor(
            &source,
            entry,
            StoredEncryptionDeclaration::raw_value,
            "rawWord",
            value,
        )?,
        EntryIssue::EncryptionBlockSize { size } => descriptor(
            &source,
            entry,
            StoredEncryptionDeclaration::declared_block_len,
            "declaredBlockLength",
            size,
        )?,
        _ => source.origin_when(&[], false),
    };
    Ok((path_origin, check_origin))
}

fn v2_data_range_origin(
    archive: &Archive,
    position: usize,
    source: &EntryDirectorySource,
    uses: &SourceUses,
) -> Result<FieldOrigin, SourceEvidenceError> {
    let Some(bounds) = archive.observed_v2_data_outside_file(position) else {
        return Ok(source.origin_when(&[], false));
    };
    let Some(root) = uses.root_directory_scope(archive)? else {
        return Ok(source.origin_when(&[], false));
    };
    let role = SourceUseRole::Conversion;
    Ok(source.origin(&[
        source.directory_subresource(
            directory_range(
                "entryDirectoryRecord",
                "data_offset",
                bounds.data_offset().range(),
            ),
            role,
        )?,
        source.directory_subresource(
            directory_range(
                "entryDirectoryRecord",
                "compressed_size",
                bounds.compressed_size().range(),
            ),
            role,
        )?,
        root.at(
            directory_range(
                "archiveV2Trailer",
                "v2/payloadEnd",
                bounds.payload_end().range(),
            ),
            role,
        )?,
    ]))
}

fn recovery_data_range_origin(
    bounds: ObservedRecoveryDataOutsideFile,
    source: &EntryDirectorySource,
) -> Result<FieldOrigin, SourceEvidenceError> {
    let mut contributors = Vec::new();
    for (namespace, field, word) in [
        (
            "entryDirectoryRecord",
            "local_header_offset",
            bounds.local_header_offset(),
        ),
        ("zipLocalHeader", "nameLength", bounds.local_name_length()),
        ("zipLocalHeader", "extraLength", bounds.local_extra_length()),
        (
            "entryDirectoryRecord",
            "compressed_size",
            bounds.compressed_size(),
        ),
    ] {
        contributors.push(source.directory_subresource(
            directory_range(namespace, field, word.range()),
            SourceUseRole::Conversion,
        )?);
    }
    contributors.push(source.directory_subresource(
        directory_range(
            "zipRecoveryBoundary",
            "centralHeaderSignature",
            bounds.candidate_signature(),
        ),
        SourceUseRole::Selector,
    )?);
    let selection = bounds.zip64_selection();
    if let Some(selection) = selection {
        let coordinates = [
            ("sizeMarkers", Some(selection.size_words())),
            ("localOffsetMarker", Some(selection.local_offset_word())),
            ("extraLength", Some(selection.extra_length_word())),
            ("winningHeader", selection.header()),
        ];
        for (field, range) in coordinates
            .into_iter()
            .filter_map(|(field, range)| range.map(|range| (field, range)))
        {
            contributors.push(source.directory_subresource(
                directory_range("zip64OperandSelection", field, range),
                SourceUseRole::Selector,
            )?);
        }
    }
    Ok(source.origin_when(
        &contributors,
        selection.is_none_or(ObservedRecoveryZip64Selection::complete),
    ))
}

fn hash_block_origin(
    archive: &Archive,
    position: usize,
    source: &EntryDirectorySource,
) -> Result<FieldOrigin, SourceEvidenceError> {
    let Some(header) = archive.observed_hash_block_size(position) else {
        return Ok(source.origin_when(&[], false));
    };
    Ok(source.origin(&[
        source.directory_subresource(
            directory_range("zipExtraBlockHeader", "blockId", header.id()),
            SourceUseRole::Selector,
        )?,
        source.directory_subresource(
            directory_range(
                "zipExtraBlockHeader",
                "declaredBlockLength",
                header.declared_len(),
            ),
            SourceUseRole::Conversion,
        )?,
    ]))
}

fn descriptor(
    source: &EntryDirectorySource,
    entry: &Entry,
    original: impl Fn(&StoredEncryptionDeclaration) -> Option<u16>,
    field: &str,
    reported: u16,
) -> Result<FieldOrigin, SourceEvidenceError> {
    // The parser reports the first defect of each kind using the exact stored word or length.
    let mut declarations = entry.metadata().encryption_declarations().iter();
    let Some(ordinal) =
        declarations.position(|declaration| original(declaration) == Some(reported))
    else {
        return Ok(source.origin_when(&[], false));
    };
    let mut contributors = Vec::new();
    for before in 0..ordinal {
        contributors.push(source.directory_fact(
            &format!("encryptionDeclarations[{before}]"),
            SourceUseRole::Selector,
        )?);
    }
    contributors.push(source.directory_fact(
        &format!("encryptionDeclarations[{ordinal}].{field}"),
        SourceUseRole::Conversion,
    )?);
    Ok(source.origin(&contributors))
}
