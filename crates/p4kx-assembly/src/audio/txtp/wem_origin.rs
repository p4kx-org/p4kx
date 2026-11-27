// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_audio::EmbeddedWemSource;
use p4kx_extraction::{
    Archive, InputScope, SourceInput, SourceLocation, SourceScope, SourceUse, SourceUseRole,
    SourceUses,
};

use crate::audio::ExportedWemOrigin;

pub(super) fn admission_uses(
    archive: &Archive,
    sources: &SourceUses,
    media: &ExportedWemOrigin,
    actual: &mut Vec<SourceUse>,
) -> io::Result<()> {
    let Some(entry) = media.entry(archive) else {
        return Ok(());
    };
    let scope = sources
        .observe(SourceInput::ArchiveView(entry), InputScope::Payload)
        .map_err(io::Error::other)?;
    let range = media.byte_range();
    actual.push(bind(
        &scope,
        "wemBytes",
        vec![range.start, range.end],
        "mediaPayload",
        SourceUseRole::Selector,
    )?);
    if let Some(embedded) = media.embedded() {
        embedded_selectors(&scope, embedded, actual)?;
    }
    Ok(())
}

pub(super) fn embedded_selectors(
    scope: &SourceScope,
    embedded: EmbeddedWemSource,
    actual: &mut Vec<SourceUse>,
) -> io::Result<()> {
    for (namespace, offset, fields) in [
        (
            "bnkDidx",
            embedded.triplet_offset(),
            ["mediaId", "dataOffset", "dataLength"].as_slice(),
        ),
        (
            "bnkSection",
            embedded.data_body_offset(),
            ["tag", "bodyLength"].as_slice(),
        ),
    ] {
        for field in fields {
            actual.push(bind(
                scope,
                namespace,
                vec![offset],
                field,
                SourceUseRole::Selector,
            )?);
        }
    }
    Ok(())
}

pub(super) fn bind(
    scope: &SourceScope,
    namespace: &str,
    indices: Vec<u64>,
    name: &str,
    role: SourceUseRole,
) -> io::Result<SourceUse> {
    scope
        .at(
            SourceLocation::Subresource {
                namespace: namespace.to_owned(),
                indices,
                field: name.to_owned(),
            },
            role,
        )
        .map_err(io::Error::other)
}
