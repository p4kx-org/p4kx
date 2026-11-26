// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::TryReserveError;
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_extraction::cryxml::{XmlRenderKind, XmlRenderSpan};
use p4kx_extraction::{ObservedEntry, SourceEvidenceError, XmlRenderOrigins};
use thiserror::Error;

/// The converted XML source evidence could not be retained.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum XmlPreviewOriginError {
    /// The positioned renderer ran out of storage for source locations.
    #[error("cannot retain XML source positions")]
    Positions(#[from] TryReserveError),
    /// The positions could not be bound to original source uses.
    #[error(transparent)]
    Source(#[from] SourceEvidenceError),
}

pub(super) struct PositionedXml(Result<Vec<XmlRenderSpan>, TryReserveError>);

impl PositionedXml {
    pub(super) const fn new() -> Self {
        Self(Ok(Vec::new()))
    }

    pub(super) fn observe(&mut self, span: XmlRenderSpan) {
        if matches!(span.kind(), XmlRenderKind::Structure)
            && span.complete()
            && span.child_absence().is_none()
            && span.sources().next().is_none()
        {
            return;
        }
        if let Ok(spans) = &mut self.0 {
            match spans.try_reserve(1) {
                Ok(()) => spans.push(span),
                Err(error) => self.0 = Err(error),
            }
        }
    }

    pub(super) fn finish(
        self,
        origin: Option<ObservedEntry<'_>>,
        retained_end: u64,
        dropped: &AtomicBool,
    ) -> Result<XmlRenderOrigins, XmlPreviewOriginError> {
        let current = self
            .0?
            .into_iter()
            .take_while(|_| !dropped.load(Ordering::Acquire));
        Ok(XmlRenderOrigins::retained(origin, current, retained_end)?)
    }
}
