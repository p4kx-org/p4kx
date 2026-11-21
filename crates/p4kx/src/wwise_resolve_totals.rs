// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::audio::BreakReason;
use p4kx_extraction::{FieldOrigin, FieldPath};

use super::variant_origins::VariantOrigins;
use super::{BreakOrigins, Capture, field};

#[derive(Clone, Copy)]
pub(super) enum SummaryFields {
    Counts,
    Nested,
}

#[derive(Default)]
pub(super) struct Totals {
    variants: Vec<FieldOrigin>,
    routes: Vec<FieldOrigin>,
    media: Vec<FieldOrigin>,
    locations: Vec<FieldOrigin>,
    breaks: Vec<FieldOrigin>,
    reasons: Vec<FieldOrigin>,
    cutoff: Option<FieldOrigin>,
}

impl Totals {
    pub(super) fn variant(&mut self, proof: VariantOrigins) {
        self.variants.push(proof.row);
        self.routes.push(proof.route);
        self.media.push(proof.media);
        self.locations.push(proof.location);
    }

    pub(super) fn broken_link(&mut self, proof: BreakOrigins, reason: &BreakReason) {
        if reason.truncated_by_limit() {
            self.cutoff = Some(proof.reason.clone());
        }
        self.breaks.push(proof.row);
        self.reasons.push(proof.reason);
    }

    pub(super) fn write(
        self,
        capture: &mut Capture<'_>,
        root: &FieldPath,
        shape: SummaryFields,
    ) -> Result<FieldOrigin> {
        let original = [&capture.definition.selection, &capture.definition.topology];
        let broken = capture.union(original.into_iter().chain(&self.reasons), true)?;
        let visited = capture.union(
            original.into_iter().chain(&self.routes).chain([&broken]),
            true,
        )?;
        if matches!(shape, SummaryFields::Counts) {
            self.counts(capture, root, &visited, &broken)?;
        }
        let variants = if self.variants.is_empty() {
            visited.clone()
        } else {
            capture.union(&self.variants, true)?
        };
        let breaks = if self.breaks.is_empty() {
            broken
        } else {
            capture.union(&self.breaks, true)?
        };
        capture.group(
            root,
            [
                ("variants", variants),
                ("breaks", breaks),
                ("truncated_by_limits", self.cutoff.unwrap_or(visited)),
            ],
        )
    }

    fn counts(
        &self,
        capture: &mut Capture<'_>,
        root: &FieldPath,
        visited: &FieldOrigin,
        broken: &FieldOrigin,
    ) -> Result<()> {
        let media = capture.union(std::iter::once(visited).chain(&self.media), true)?;
        let locations = capture.union(std::iter::once(visited).chain(&self.locations), true)?;
        let counts = capture.group(
            &field(root, "counts")?,
            [
                ("variants", visited.clone()),
                ("located", locations.clone()),
                ("unlocated", capture.union([&locations, &media], true)?),
                ("plugins", media),
                ("breaks", broken.clone()),
            ],
        )?;
        capture.put(root, "counts", counts)
    }
}
