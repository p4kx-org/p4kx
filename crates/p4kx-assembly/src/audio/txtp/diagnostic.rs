// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::audio::{HircPathSource, SoundVariant};

use super::TxtpApproximation;
use super::render::RenderedTxtp;
use super::selection::{self, SelectionDiagnostic};

pub(super) struct TxtpDiagnostic<'a> {
    pub(super) approximation: TxtpApproximation,
    pub(super) header_bytes: Range<usize>,
    pub(super) source: Option<DiagnosticSource<'a>>,
}

pub(super) enum DiagnosticSource<'a> {
    Authored(&'a HircPathSource),
    Selection(SelectionDiagnostic<'a>),
}

pub(super) fn collect<'a>(
    rendered: &'a RenderedTxtp<'a>,
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
            source: match approximation {
                TxtpApproximation::SelectFirstAlternative => rendered
                    .selection
                    .as_ref()
                    .and_then(|commands| rendered.commands.get(commands.clone()))
                    .map(|commands| selection::resolve(variants, commands))
                    .map(DiagnosticSource::Selection),
                _ => variants
                    .iter()
                    .flat_map(SoundVariant::path_sources)
                    .find(|source| source.diagnostic_site(approximation).is_some())
                    .map(DiagnosticSource::Authored),
            },
        })
        .collect()
}
