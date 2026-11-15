// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::audio::{HircPathSource, SoundVariant};

use super::TxtpApproximation;
use super::render::RenderedTxtp;

pub(super) struct TxtpDiagnostic<'a> {
    pub(super) approximation: TxtpApproximation,
    pub(super) header_bytes: Range<usize>,
    pub(super) source: Option<&'a HircPathSource>,
}

pub(super) fn collect<'a>(
    rendered: &RenderedTxtp<'_>,
    variants: &'a [SoundVariant<'_>],
) -> Vec<TxtpDiagnostic<'a>> {
    rendered
        .approximations
        .iter()
        .copied()
        .zip(&rendered.approximation_ranges)
        .map(|(approximation, bytes)| TxtpDiagnostic {
            approximation,
            header_bytes: bytes.clone(),
            source: variants
                .iter()
                .flat_map(SoundVariant::path_sources)
                .find(|source| source.diagnostic_site(approximation).is_some()),
        })
        .collect()
}
