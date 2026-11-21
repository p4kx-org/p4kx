// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::audio::{AudioIndex, LanguageChoice};
use p4kx_extraction::{
    Archive, FieldOrigin, InputScope, ObservedEntry, SourceCoordinateOmission, SourceInput,
    SourceLocation, SourceUseRole, SourceUses,
};

use super::{Input, Output};

pub(super) fn write(output: &mut Output, input: &Input<'_, '_>) -> Result<FieldOrigin> {
    output.put("/archive", output.uses.origin(&[], true))?;
    let entry = output
        .scope
        .for_scope(InputScope::ArchiveDirectoryMetadata)
        .at(
            SourceLocation::Metadata {
                field: "path".into(),
            },
            SourceUseRole::Payload,
        )?;
    output.put(
        "/entry",
        output.uses.origin(&[entry], output.dataforge_complete),
    )?;
    let record = input.record.index();
    let name = input.record.name().is_ok();
    let mut fields = Vec::new();
    for (member, original, role, readable) in [
        ("guid", "guid", SourceUseRole::Payload, true),
        (
            "record_path",
            "fileName",
            SourceUseRole::Conversion,
            input.record.record_path().is_ok(),
        ),
        ("name", "recordName", SourceUseRole::Payload, name),
        ("class_name", "recordName", SourceUseRole::Conversion, name),
    ] {
        let site = output.scope.at(
            SourceLocation::DataForge {
                record_index: record,
                field: original.into(),
            },
            role,
        )?;
        let field = output
            .uses
            .origin(&[site], readable && output.dataforge_complete);
        output.put(&format!("/entity/{member}"), field.clone())?;
        fields.push(field);
    }
    output.put("/entity", output.union(&fields, true)?)?;
    if let Some(choice) = input.language {
        write_language(output, input, choice)?;
    }
    let inspection = output.scope.at(
        SourceLocation::Subresource {
            namespace: "dataforgeRecordTable".into(),
            indices: vec![u64::from(record)],
            field: "canonicalRecord".into(),
        },
        SourceUseRole::Selector,
    )?;
    Ok(output.uses.origin(
        &[inspection],
        output.dataforge_complete && input.inspection.complete,
    ))
}

fn write_language(
    output: &mut Output,
    input: &Input<'_, '_>,
    choice: &LanguageChoice,
) -> Result<()> {
    let canonical = |name| {
        directory(
            &output.uses,
            input.index.language_source(input.archive, name),
            SourceUseRole::Conversion,
        )
    };
    let requested = choice
        .preferred()
        .map_or_else(|| Ok(output.uses.origin(&[], true)), canonical)?;
    let fallback = choice.fallback().map_or_else(
        || {
            let paths = output.uses.observe_root_directory_paths(input.archive)?;
            Ok(output.uses.origin(paths.as_slice(), paths.is_some()))
        },
        canonical,
    )?;
    let origin = output.members(
        "/language",
        [("requested", requested), ("fallback", fallback)],
    )?;
    output.put("/language", origin)
}

pub(super) fn issues(output: &mut Output, archive: &Archive, index: &AudioIndex) -> Result<()> {
    let mut rows = Vec::new();
    for (ordinal, issue) in index.issues().iter().enumerate() {
        let entry = issue.source(archive);
        let path = directory(&output.uses, entry, SourceUseRole::Payload)?;
        let diagnostic = if let Some(entry) = entry {
            let scope = output
                .uses
                .observe(SourceInput::ArchiveView(entry), InputScope::Payload)?;
            let attempted = scope.at_opaque(
                SourceUseRole::Attempted,
                SourceCoordinateOmission::Unavailable,
            )?;
            output.uses.origin(&[attempted], false)
        } else {
            output.uses.origin(&[], false)
        };
        let address = format!("/issues/{ordinal}");
        let origin = output.members(
            &address,
            [
                ("path", path),
                ("kind", diagnostic.clone()),
                ("message", diagnostic),
            ],
        )?;
        output.put(&address, origin.clone())?;
        rows.push(origin);
    }
    output.put("/issues", output.union(&rows, true)?)
}

pub(super) fn directory(
    uses: &SourceUses,
    entry: Option<ObservedEntry<'_>>,
    role: SourceUseRole,
) -> Result<FieldOrigin> {
    let Some(entry) = entry else {
        return Ok(uses.origin(&[], false));
    };
    let scope = uses.observe(
        SourceInput::ArchiveView(entry),
        InputScope::ArchiveDirectoryMetadata,
    )?;
    let path = scope.at(
        SourceLocation::Metadata {
            field: "path".into(),
        },
        role,
    )?;
    Ok(uses.origin(
        &[path],
        entry.ancestors().all(|ancestor| ancestor.is_some()),
    ))
}
