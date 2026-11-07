// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Compression, EffectiveEncoding, Entry};

use super::bytes::Verdict;

/// The inspected unmodified producer revision whose trim algorithm is supported.
pub(crate) const PRODUCER_REVISION: &str = "b492ab14d26280c6ec91c4365ff0faf5f3e24a6b";

pub(crate) const TRAILING_ZEROS: &str =
    "validated unp4k b492ab14: encrypted direct bytes, exact pre-decompression zero trim";

/// The Entry identity verified against the reference run's source manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceIdentity<'a> {
    pub(crate) path: &'a str,
    pub(crate) compression: Compression,
    pub(crate) encrypted: bool,
}

impl<'a> SourceIdentity<'a> {
    pub(crate) fn of(entry: &'a Entry) -> Self {
        Self {
            path: entry.path(),
            compression: entry.compression(),
            encrypted: entry.is_encrypted(),
        }
    }
}

/// Producer and source evidence supplied only after reference-manifest validation.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Context<'a> {
    pub(crate) revision: &'a str,
    pub(crate) patch_scope: Option<&'a str>,
    pub(crate) source: SourceIdentity<'a>,
}

pub(crate) fn allows(
    context: Option<&Context<'_>>,
    source: SourceIdentity<'_>,
    encoding: EffectiveEncoding,
    verdict: Verdict,
) -> bool {
    let Some(context) = context else {
        return false;
    };
    if context.revision != PRODUCER_REVISION
        || context.patch_scope.is_some()
        || context.source != source
        || !source.encrypted
    {
        return false;
    }
    if !matches!(
        (source.compression, encoding),
        (Compression::Store, EffectiveEncoding::Stored)
            | (Compression::Zstd, EffectiveEncoding::PassThrough)
    ) {
        return false;
    }
    let Verdict::ReferenceShorter {
        zeros_only: true,
        extra,
        reference_len,
        last_reference_byte,
    } = verdict
    else {
        return false;
    };
    extra > 0
        && (reference_len == 1 && last_reference_byte.is_some()
            || (reference_len >= 3 && last_reference_byte.is_some_and(|byte| byte != 0)))
}
