// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Compression, EffectiveEncoding, Entry};

use super::bytes::Verdict;

/// The inspected unmodified producer revision whose trim algorithm is supported.
pub(crate) const PRODUCER_REVISION: &str = "b492ab14d26280c6ec91c4365ff0faf5f3e24a6b";

pub(crate) const TRAILING_ZEROS: &str =
    "validated unp4k b492ab14: encrypted direct bytes, exact pre-decompression zero trim";

const MIN_TRIMMED_NONZERO_PREFIX: u64 = 3;
const METHOD_100_PROBE_BYTES: u64 = 4;

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
    let Verdict::ReferenceShorter {
        zeros_only: true,
        extra,
        reference_len,
        last_reference_byte,
    } = verdict
    else {
        return false;
    };
    if extra == 0 {
        return false;
    }
    match (source.compression, encoding, last_reference_byte) {
        (Compression::Store, EffectiveEncoding::Stored, Some(last)) => {
            reference_len == 1 || (reference_len >= MIN_TRIMMED_NONZERO_PREFIX && last != 0)
        }
        (Compression::Zstd, EffectiveEncoding::PassThrough, Some(last)) => {
            reference_len >= METHOD_100_PROBE_BYTES && last != 0
        }
        _ => false,
    }
}
