// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{InputScope, SourceUseRole};

use crate::origin_capture::OriginCapture;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_resolver::OriginResolver;
use crate::origin_source;
use crate::{Identity, MediaError};

const MEMBERS: &[&str] = &["entry_index", "path", "raw_path_hex"];

pub(crate) fn resolve(
    resolver: &OriginResolver<'_>,
    identity: &Identity,
    field: &str,
) -> Result<Option<OriginContributions>, MediaError> {
    match field {
        "/identity/type" => kind(resolver).map(Some),
        "/identity" => whole(resolver, identity).map(Some),
        "/identity/containers" => containers(resolver, identity).map(Some),
        "/identity/entry" => container(resolver, resolver.source, None).map(Some),
        _ => component(resolver, field),
    }
}

fn whole(
    resolver: &OriginResolver<'_>,
    identity: &Identity,
) -> Result<OriginContributions, MediaError> {
    let mut capture = OriginCapture::with_limit(resolver.owner, resolver.detail_bytes);
    capture.include_original(&kind(resolver)?)?;
    if matches!(identity, Identity::Entry { .. }) {
        capture.include_original(&containers(resolver, identity)?)?;
        capture.include_original(&container(resolver, resolver.source, None)?)?;
    }
    Ok(capture.finish())
}

fn containers(
    resolver: &OriginResolver<'_>,
    identity: &Identity,
) -> Result<OriginContributions, MediaError> {
    let mut capture = OriginCapture::with_limit(resolver.owner, resolver.detail_bytes);
    if let Identity::Entry { containers, .. } = identity {
        for position in 0..containers.len() {
            let ancestor = resolver
                .source
                .map(|input| input.ancestor(position))
                .transpose()?
                .flatten();
            capture.include_original(&container(resolver, ancestor.as_ref(), None)?)?;
        }
    }
    Ok(capture.finish())
}

fn component(
    resolver: &OriginResolver<'_>,
    field: &str,
) -> Result<Option<OriginContributions>, MediaError> {
    if let Some(member) = field.strip_prefix("/identity/entry/") {
        return container(resolver, resolver.source, Some(member)).map(Some);
    }
    let Some(suffix) = field.strip_prefix("/identity/containers/") else {
        return Ok(None);
    };
    let (ordinal, member) = suffix
        .split_once('/')
        .map_or((suffix, None), |(ordinal, field)| (ordinal, Some(field)));
    let Some(position) = ordinal.parse::<usize>().ok() else {
        return Ok(Some(OriginContributions::unknown(resolver.owner)));
    };
    let ancestor = resolver
        .source
        .map(|input| input.ancestor(position))
        .transpose()?
        .flatten();
    container(resolver, ancestor.as_ref(), member).map(Some)
}

fn container(
    resolver: &OriginResolver<'_>,
    input: Option<&MediaOriginInput>,
    member: Option<&str>,
) -> Result<OriginContributions, MediaError> {
    let Some(input) = input else {
        return Ok(OriginContributions::unknown(resolver.owner));
    };
    if let Some(member) = member {
        return origin_source::intrinsic(
            resolver,
            input,
            InputScope::ArchiveDirectoryMetadata,
            member,
            SourceUseRole::Payload,
        );
    }
    let mut capture = OriginCapture::with_limit(resolver.owner, resolver.detail_bytes);
    for member in MEMBERS {
        capture.include_original(&origin_source::intrinsic(
            resolver,
            input,
            InputScope::ArchiveDirectoryMetadata,
            member,
            SourceUseRole::Payload,
        )?)?;
    }
    Ok(capture.finish())
}
fn kind(resolver: &OriginResolver<'_>) -> Result<OriginContributions, MediaError> {
    resolver.source.map_or_else(
        || Ok(OriginContributions::unknown(resolver.owner)),
        |input| {
            origin_source::intrinsic(
                resolver,
                input,
                InputScope::ArchiveDirectoryMetadata,
                "physical-identity",
                SourceUseRole::Payload,
            )
        },
    )
}
