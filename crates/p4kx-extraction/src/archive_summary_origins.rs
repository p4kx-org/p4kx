// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, ArchiveMetadataStatus, BuildManifestOutcome};

use crate::source_provenance::archive_layout_origins;
use crate::{
    ArchiveLayoutOriginError, ArchiveLayoutOrigins, ArchiveSummary, FieldOrigin, FieldOrigins,
    FieldPath, InputScope, SourceInput, SourceLocation, SourceUseRole, SourceUses,
};

const BUILD_FIELDS: [(&str, &str); 4] = [
    ("branch", "Data.Branch"),
    ("version", "Data.Version"),
    ("change_number", "Data.RequestedP4ChangeNum"),
    ("build_id", "Data.BuildId"),
];

pub(super) fn capture(
    archive: &Archive,
    summary: &ArchiveSummary,
    address: &str,
) -> Result<ArchiveLayoutOrigins, ArchiveLayoutOriginError> {
    let uses = SourceUses::with_compact_use_refs();
    let mut fields = FieldOrigins::new();
    let mut path = FieldPath::parse(address)?;
    path.push_field("metadata")?;
    archive_layout_origins::write(archive, &summary.metadata, &uses, &mut fields, &mut path)?;
    path.pop();
    write_directory(archive, summary, &uses, &mut fields, &mut path)?;
    path.push_field("build")?;
    write_build(archive, &uses, &mut fields, &mut path)?;
    Ok(ArchiveLayoutOrigins {
        field_origins: fields,
        provenance: uses.freeze()?,
    })
}

fn write_directory(
    archive: &Archive,
    summary: &ArchiveSummary,
    uses: &SourceUses,
    fields: &mut FieldOrigins,
    path: &mut FieldPath,
) -> Result<(), ArchiveLayoutOriginError> {
    let complete = archive.metadata().status() != ArchiveMetadataStatus::Recovered;
    let entry_count = uses.observe_root_directory_entry_count(archive)?;
    put(
        fields,
        path,
        "entry_count",
        uses.origin(entry_count.as_slice(), complete && entry_count.is_some()),
    )?;
    let scope = uses.root_directory_scope(archive)?;
    for (key, original) in [
        ("encrypted_count", "encryption_declarations"),
        ("compressed_size", "compressed_size"),
        ("size", "uncompressed_size"),
    ] {
        let observed = scope
            .as_ref()
            .map(|scope| {
                scope.at(
                    SourceLocation::Subresource {
                        namespace: "archiveDirectoryAdmittedFields".into(),
                        indices: vec![0, summary.entry_count as u64],
                        field: original.into(),
                    },
                    SourceUseRole::Conversion,
                )
            })
            .transpose()?;
        let origin = uses.origin(observed.as_slice(), complete && observed.is_some());
        put(fields, path, key, origin)?;
    }
    Ok(())
}

fn write_build(
    archive: &Archive,
    uses: &SourceUses,
    fields: &mut FieldOrigins,
    path: &mut FieldPath,
) -> Result<(), ArchiveLayoutOriginError> {
    let manifest = archive.build_manifest_observation();
    let Some(input) = manifest.origin() else {
        fields.insert(path.clone(), uses.origin(&[], false))?;
        return Ok(());
    };
    let scope = uses.observe(SourceInput::BuildManifest(input), InputScope::Payload)?;
    match manifest.outcome() {
        BuildManifestOutcome::Parsed => {
            let required_shape =
                scope.at(manifest_field("required-shape"), SourceUseRole::Selector)?;
            let mut aggregate = vec![required_shape.clone()];
            for (key, original) in BUILD_FIELDS {
                let payload = scope.at(manifest_field(original), SourceUseRole::Payload)?;
                let conversion = scope.at(manifest_field(original), SourceUseRole::Conversion)?;
                let contributors = [payload, conversion, required_shape.clone()];
                put(fields, path, key, uses.origin(&contributors, true))?;
                aggregate.extend(contributors);
            }
            let selector = scope.at(
                manifest_field("Data.RequestedP4ChangeNum"),
                SourceUseRole::Selector,
            )?;
            let patch_origin = uses.origin(std::slice::from_ref(&selector), false);
            put(fields, path, "public_patch", patch_origin)?;
            aggregate.push(selector);
            fields.insert(path.clone(), uses.origin(&aggregate, false))?;
        }
        BuildManifestOutcome::Malformed => {
            let location = manifest_field("parse-outcome");
            let selector = scope.at(location.clone(), SourceUseRole::Selector)?;
            let conversion = scope.at(location, SourceUseRole::Conversion)?;
            fields.insert(path.clone(), uses.origin(&[selector, conversion], true))?;
        }
        _ => fields.insert(path.clone(), uses.origin(&[], false))?,
    }
    Ok(())
}

fn manifest_field(field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: "build-manifest".into(),
        indices: Vec::new(),
        field: field.into(),
    }
}

fn put(
    fields: &mut FieldOrigins,
    path: &mut FieldPath,
    key: &str,
    origin: FieldOrigin,
) -> Result<(), ArchiveLayoutOriginError> {
    path.push_field(key)?;
    fields.insert(path.clone(), origin)?;
    path.pop();
    Ok(())
}
