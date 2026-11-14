// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{InputScope, SourceUseRole};

use crate::origin_capture::{OriginCapture, OriginSite};
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_resolver::{self, OriginResolver};
use crate::origin_source_identity;
use crate::{MediaError, Source};

const MEMBERS: &[&str] = &[
    "/root",
    "/identity",
    "/size",
    "/declared_integrity",
    "/verification",
    "/raw",
];

pub(crate) fn resolve(
    resolver: &OriginResolver<'_>,
    pointer: &str,
) -> Result<Option<OriginContributions>, MediaError> {
    let Some(field) = pointer.strip_prefix(resolver.source_prefix) else {
        return Ok(None);
    };
    if !field.is_empty() && !field.starts_with('/') {
        return Ok(None);
    }
    let Some(source) = resolver.subject.source_data() else {
        return Ok(None);
    };
    if field.is_empty() {
        return whole(resolver).map(Some);
    }
    member(resolver, source, field).map(Some)
}

pub(crate) fn whole(resolver: &OriginResolver<'_>) -> Result<OriginContributions, MediaError> {
    let Some(source) = resolver.subject.source_data() else {
        return Ok(OriginContributions::unknown(resolver.owner));
    };
    let mut capture = OriginCapture::with_limit(resolver.owner, resolver.detail_bytes);
    for field in MEMBERS {
        capture.include_original(&member(resolver, source, field)?)?;
    }
    Ok(capture.finish())
}

fn member(
    resolver: &OriginResolver<'_>,
    source: &Source,
    field: &str,
) -> Result<OriginContributions, MediaError> {
    if origin_resolver::is_computed_digest_descriptor(field, "/root")
        || origin_resolver::is_computed_digest_descriptor(field, "/raw/digest")
    {
        return Ok(OriginContributions::independent(resolver.owner));
    }
    if origin_resolver::within(field, "/root") {
        return resolver.root.map_or_else(
            || Ok(OriginContributions::unknown(resolver.owner)),
            |input| {
                intrinsic(
                    resolver,
                    input,
                    InputScope::Payload,
                    "complete-archive-file",
                    SourceUseRole::Conversion,
                )
            },
        );
    }
    if let Some(identity) = origin_source_identity::resolve(resolver, &source.identity, field)? {
        return Ok(identity);
    }
    match field {
        "/verification" if source.raw.is_none() => {
            Ok(OriginContributions::independent(resolver.owner))
        }
        "/verification" => verification(resolver),
        "/size" => metadata(resolver, "uncompressedSize"),
        _ if origin_resolver::within(field, "/raw") => raw(resolver, source),
        _ if origin_resolver::within(field, "/declared_integrity") => {
            if source.declared_integrity.is_none() {
                Ok(OriginContributions::independent(resolver.owner))
            } else {
                metadata(resolver, "declaredChecksum")
            }
        }
        _ => Ok(OriginContributions::unknown(resolver.owner)),
    }
}

fn raw(resolver: &OriginResolver<'_>, source: &Source) -> Result<OriginContributions, MediaError> {
    if source.raw.is_none() {
        return Ok(OriginContributions::independent(resolver.owner));
    }
    resolver.source.map_or_else(
        || Ok(OriginContributions::unknown(resolver.owner)),
        |input| {
            intrinsic(
                resolver,
                input,
                InputScope::Payload,
                "complete-payload",
                SourceUseRole::Conversion,
            )
        },
    )
}

fn verification(resolver: &OriginResolver<'_>) -> Result<OriginContributions, MediaError> {
    let Some(input) = resolver.source else {
        return Ok(OriginContributions::unknown(resolver.owner));
    };
    let mut capture = OriginCapture::with_limit(resolver.owner, resolver.detail_bytes);
    for scope in [InputScope::Payload, InputScope::ArchiveDirectoryMetadata] {
        capture.include_original(&intrinsic(
            resolver,
            input,
            scope,
            "directory-check",
            SourceUseRole::Conversion,
        )?)?;
    }
    Ok(capture.finish())
}

fn metadata(resolver: &OriginResolver<'_>, field: &str) -> Result<OriginContributions, MediaError> {
    resolver.source.map_or_else(
        || Ok(OriginContributions::unknown(resolver.owner)),
        |input| {
            intrinsic(
                resolver,
                input,
                InputScope::ArchiveDirectoryMetadata,
                field,
                SourceUseRole::Payload,
            )
        },
    )
}

pub(crate) fn intrinsic(
    resolver: &OriginResolver<'_>,
    input: &MediaOriginInput,
    scope: InputScope,
    field: &str,
    role: SourceUseRole,
) -> Result<OriginContributions, MediaError> {
    let mut capture = OriginCapture::with_limit(resolver.owner, resolver.detail_bytes);
    capture.add_original(
        input,
        scope,
        OriginSite::new("media-physical-input", &[], field),
        role,
    )?;
    Ok(capture.finish())
}
